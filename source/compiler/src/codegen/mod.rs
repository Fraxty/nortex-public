//! Windows x64 COFF code generation for the validated scalar, string, and class MIR subset.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::str::FromStr;

use cranelift_codegen::ir::{
    AbiParam, Block, InstBuilder, MemFlags, Signature, TrapCode, UserFuncName,
    condcodes::{FloatCC, IntCC},
    types,
};
use cranelift_codegen::isa;
use cranelift_codegen::settings;
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext, Variable};
use cranelift_module::{DataDescription, DataId, FuncId, Linkage, Module, default_libcall_names};
use cranelift_object::{ObjectBuilder, ObjectModule};

use crate::builtins::{self, Builtin};
use crate::driver::LoweredMirFunction;
use crate::mir::{
    BinaryOp, LocalId, MirBody, Operand, Place, ProjectionElem, Rvalue, StatementKind,
    TerminatorKind,
};

#[derive(Debug, Clone, Copy)]
pub enum Target {
    X86_64Windows,
}

pub struct CodeGenerator {
    pub target: Target,
    /// Trap on signed `int`/`long` add/sub/mul overflow instead of wrapping.
    pub overflow_checks: bool,
}

impl CodeGenerator {
    pub fn new(target: Target) -> Self {
        Self {
            target,
            overflow_checks: false,
        }
    }

    pub fn with_overflow_checks(mut self, enabled: bool) -> Self {
        self.overflow_checks = enabled;
        self
    }

    pub fn emit_object(&self, functions: &[LoweredMirFunction]) -> Result<Vec<u8>, String> {
        match self.target {
            Target::X86_64Windows => emit_windows_x64_object(functions, self.overflow_checks),
        }
    }

    pub fn requires_runtime(functions: &[LoweredMirFunction]) -> bool {
        uses_console_output(functions)
            || uses_string_concat(functions)
            || uses_string_eq(functions)
            || uses_object_allocation(functions)
            || !used_builtins(functions).is_empty()
    }
}

fn emit_windows_x64_object(
    functions: &[LoweredMirFunction],
    overflow_checks: bool,
) -> Result<Vec<u8>, String> {
    if functions.is_empty() {
        return Err("Es gibt keine MIR-Funktionen für die Codegenerierung.".to_owned());
    }

    let triple = target_lexicon::Triple::from_str("x86_64-pc-windows-msvc")
        .map_err(|error| format!("Windows-x64-Target ist ungültig: {error}"))?;
    let isa_builder = isa::lookup(triple)
        .map_err(|error| format!("Cranelift unterstützt Windows x64 nicht: {error:?}"))?;
    let flags = settings::Flags::new(settings::builder());
    let isa = isa_builder.finish(flags).map_err(|error| {
        format!("Windows-x64-Codegenerator konnte nicht initialisiert werden: {error}")
    })?;
    let object_builder = ObjectBuilder::new(isa, "nortex".to_owned(), default_libcall_names())
        .map_err(|error| format!("COFF-Objekt konnte nicht initialisiert werden: {error}"))?;
    let mut module = ObjectModule::new(object_builder);

    let mut symbols = HashSet::new();
    let mut function_ids = HashMap::<String, FuncId>::new();
    let mut function_indices = HashMap::<String, usize>::new();
    let mut signatures = Vec::<Signature>::with_capacity(functions.len());
    let mut entry_count = 0usize;
    let mut external_functions = HashSet::<String>::new();

    for function in functions {
        if function.external {
            if function.body.local_decls[0].type_name == "string" {
                return Err(format!(
                    "Externe Funktion '{}' darf keinen string zurückgeben; C-Zeiger besitzen keinen Slice-Deskriptor.",
                    function.name
                ));
            }
            external_functions.insert(function.name.clone());
            // C-ABI: der Symbolname bleibt unverändert (kein nortex_-Präfix, keine Dekoration).
            let symbol = function.name.clone();
            if !symbols.insert(symbol.clone()) {
                return Err(format!("COFF-Symbol '{symbol}' ist nicht eindeutig."));
            }
            let signature = function_signature(&module, &function.body)?;
            let id = module
                .declare_function(&symbol, Linkage::Import, &signature)
                .map_err(|error| {
                    format!(
                        "Externe Funktion '{}' konnte nicht deklariert werden: {error}",
                        function.name
                    )
                })?;
            function_ids.insert(function.name.clone(), id);
            function_indices.insert(function.name.clone(), signatures.len());
            signatures.push(signature);
            continue;
        }
        function
            .body
            .validate()
            .map_err(|error| format!("MIR {} ist ungültig: {error}", function.name))?;
        let symbol = external_symbol_name(&function.name);
        if !symbols.insert(symbol.clone()) {
            return Err(format!("COFF-Symbol '{symbol}' ist nicht eindeutig."));
        }
        let is_entry = symbol == "main";
        if is_entry {
            entry_count += 1;
            if function.body.arg_count != 0
                || !matches!(
                    function.body.local_decls[0].type_name.as_str(),
                    "int" | "i32"
                )
            {
                return Err(
                    "Windows-Programme benötigen derzeit genau eine parameterlose int-main-Funktion."
                        .to_owned(),
                );
            }
        }

        let signature = function_signature(&module, &function.body)?;
        let id = module
            .declare_function(&symbol, Linkage::Export, &signature)
            .map_err(|error| {
                format!(
                    "Funktion '{}' konnte nicht deklariert werden: {error}",
                    function.name
                )
            })?;
        function_ids.insert(function.name.clone(), id);
        function_indices.insert(function.name.clone(), signatures.len());
        signatures.push(signature);
    }
    let runtime_console = if uses_console_output(functions) {
        let mut signature = module.make_signature();
        signature.params.extend([
            AbiParam::new(types::I64),
            AbiParam::new(types::I64),
            AbiParam::new(types::I8),
        ]);
        signature.returns.push(AbiParam::new(types::I32));
        let id = module
            .declare_function("nortex_runtime_console_write", Linkage::Import, &signature)
            .map_err(|error| {
                format!(
                    "Runtime-Funktion für Konsolenausgabe konnte nicht deklariert werden: {error}"
                )
            })?;
        Some(id)
    } else {
        None
    };
    let runtime_concat = if uses_string_concat(functions) {
        let mut signature = module.make_signature();
        signature.params.extend([AbiParam::new(types::I64); 4]);
        signature.returns.push(AbiParam::new(types::I64));
        let id = module
            .declare_function("nortex_string_concat", Linkage::Import, &signature)
            .map_err(|error| {
                format!(
                    "Runtime-Funktion für String-Konkatenation konnte nicht deklariert werden: {error}"
                )
            })?;
        Some(id)
    } else {
        None
    };
    let runtime_eq = if uses_string_eq(functions) {
        let mut signature = module.make_signature();
        signature.params.extend([AbiParam::new(types::I64); 4]);
        signature.returns.push(AbiParam::new(types::I8));
        Some(
            module
                .declare_function("nortex_string_eq", Linkage::Import, &signature)
                .map_err(|error| {
                    format!("Runtime-Funktion für String-Vergleich nicht deklarierbar: {error}")
                })?,
        )
    } else {
        None
    };
    let runtime_builtins = declare_builtin_imports(&mut module, functions)?;
    let runtime_object_shutdown = if uses_object_allocation(functions) {
        let signature = module.make_signature();
        let id = module
            .declare_function("nortex_object_arena_shutdown", Linkage::Import, &signature)
            .map_err(|error| {
                format!("Objektarena-Freigabe konnte nicht importiert werden: {error}")
            })?;
        Some(id)
    } else {
        None
    };
    // Heap-Strings entstehen durch Konkatenation und String-Builtins; sonst ist kein Drop nötig.
    let runtime_free = if runtime_concat.is_some() || !runtime_builtins.is_empty() {
        let mut signature = module.make_signature();
        signature.params.push(AbiParam::new(types::I64));
        Some(
            module
                .declare_function("nortex_string_free", Linkage::Import, &signature)
                .map_err(|error| {
                    format!("Runtime-Funktion zur String-Freigabe nicht deklarierbar: {error}")
                })?,
        )
    } else {
        None
    };
    if uses_object_allocation(functions) {
        let mut signature = module.make_signature();
        signature.params.extend([AbiParam::new(types::I64); 2]);
        signature.returns.push(AbiParam::new(types::I64));
        let id = module
            .declare_function("nortex_object_alloc", Linkage::Import, &signature)
            .map_err(|error| format!("Objektarena konnte nicht importiert werden: {error}"))?;
        let index = signatures.len();
        function_ids.insert("nortex_object_alloc".to_owned(), id);
        function_indices.insert("nortex_object_alloc".to_owned(), index);
        external_functions.insert("nortex_object_alloc".to_owned());
        signatures.push(signature);
    }
    if entry_count > 1 {
        return Err(
            "Mehr als eine Funktion wurde auf das Windows-Einstiegssymbol 'main' abgebildet."
                .to_owned(),
        );
    }

    let globals = &functions[0].body.globals;
    if functions
        .iter()
        .any(|function| function.body.globals != *globals)
    {
        return Err("MIR-Funktionen besitzen inkonsistente globale Variablen.".to_owned());
    }
    let mut global_ids = HashMap::<String, DataId>::new();
    for global in globals {
        let symbol = global_symbol_name(&global.name);
        if !symbols.insert(symbol.clone()) {
            return Err(format!("COFF-Symbol '{symbol}' ist nicht eindeutig."));
        }
        let linkage = if global.public {
            Linkage::Export
        } else {
            Linkage::Local
        };
        let data_id = module
            .declare_data(&symbol, linkage, global.mutable, false)
            .map_err(|error| {
                format!(
                    "Globalvariable '{}' konnte nicht deklariert werden: {error}",
                    global.name
                )
            })?;
        let mut data = DataDescription::new();
        if let crate::mir::MirGlobalValue::String(text) = &global.value {
            // Der Global hält einen Zeiger auf einen schreibgeschützten {ptr, len}-Deskriptor.
            let descriptor_id = define_string_descriptor(&mut module, &mut symbols, text, 0)?;
            let descriptor = module.declare_data_in_data(descriptor_id, &mut data);
            data.write_data_addr(0, descriptor, 16);
            data.set_align(8);
            data.define(vec![0; 8].into_boxed_slice());
        } else {
            let bytes = global_bytes(global)?;
            data.set_align(bytes.len().next_power_of_two() as u64);
            data.define(bytes.into_boxed_slice());
        }
        module.define_data(data_id, &data).map_err(|error| {
            format!(
                "Initialwert der Globalvariable '{}' konnte nicht gespeichert werden: {error}",
                global.name
            )
        })?;
        global_ids.insert(global.name.clone(), data_id);
    }
    let string_constants = declare_string_constants(&mut module, functions, &mut symbols)?;

    for (index, function) in functions.iter().enumerate() {
        if function.external {
            continue;
        }
        let id = function_ids[&function.name];
        let mut context = module.make_context();
        context.func.signature = signatures[index].clone();
        context.func.name = UserFuncName::user(0, id.as_u32());
        let mut builder_context = FunctionBuilderContext::new();
        let lowering_context = FunctionLoweringContext {
            body: &function.body,
            function_ids: &function_ids,
            function_indices: &function_indices,
            signatures: &signatures,
            runtime_console: runtime_console.as_ref(),
            runtime_concat: runtime_concat.as_ref(),
            runtime_eq: runtime_eq.as_ref(),
            runtime_free: runtime_free.as_ref(),
            runtime_builtins: &runtime_builtins,
            external_functions: &external_functions,
            global_ids: &global_ids,
            string_constants: &string_constants,
            is_entry: function.name == "main",
            runtime_object_shutdown: runtime_object_shutdown.as_ref(),
            overflow_checks,
        };
        lower_function(
            &mut module,
            &mut context.func,
            &mut builder_context,
            &lowering_context,
        )
        .map_err(|error| {
            format!(
                "Codegenerierung für '{}' fehlgeschlagen: {error}",
                function.name
            )
        })?;
        module.define_function(id, &mut context).map_err(|error| {
            format!(
                "Maschinencode für '{}' konnte nicht erzeugt werden: {error}",
                function.name
            )
        })?;
        module.clear_context(&mut context);
    }

    module
        .finish()
        .emit()
        .map_err(|error| format!("COFF-Objekt konnte nicht serialisiert werden: {error}"))
}

fn function_signature(module: &ObjectModule, body: &MirBody) -> Result<Signature, String> {
    let mut signature = module.make_signature();
    for parameter in body.local_decls.iter().skip(1).take(body.arg_count) {
        signature
            .params
            .push(AbiParam::new(cranelift_type(&parameter.type_name)?));
    }
    let return_type = &body.local_decls[0].type_name;
    if return_type != "void" {
        signature
            .returns
            .push(AbiParam::new(cranelift_type(return_type)?));
    }
    Ok(signature)
}

struct FunctionLoweringContext<'a> {
    body: &'a MirBody,
    function_ids: &'a HashMap<String, FuncId>,
    function_indices: &'a HashMap<String, usize>,
    signatures: &'a [Signature],
    runtime_console: Option<&'a FuncId>,
    runtime_concat: Option<&'a FuncId>,
    runtime_eq: Option<&'a FuncId>,
    runtime_free: Option<&'a FuncId>,
    runtime_builtins: &'a HashMap<&'static str, FuncId>,
    external_functions: &'a HashSet<String>,
    global_ids: &'a HashMap<String, DataId>,
    string_constants: &'a HashMap<String, DataId>,
    is_entry: bool,
    runtime_object_shutdown: Option<&'a FuncId>,
    overflow_checks: bool,
}

fn lower_function(
    module: &mut ObjectModule,
    function: &mut cranelift_codegen::ir::Function,
    builder_context: &mut FunctionBuilderContext,
    context: &FunctionLoweringContext<'_>,
) -> Result<(), String> {
    let body = context.body;
    let mut builder = FunctionBuilder::new(function, builder_context);
    let blocks = body
        .basic_blocks
        .iter()
        .map(|_| builder.create_block())
        .collect::<Vec<_>>();
    let entry = blocks[0];
    builder.append_block_params_for_function_params(entry);

    let variable_types = body
        .local_decls
        .iter()
        .enumerate()
        .map(|(index, local)| {
            if index == 0 && local.type_name == "void" {
                Ok(None)
            } else {
                cranelift_type(&local.type_name).map(Some)
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    for (index, ty) in variable_types.iter().enumerate() {
        if let Some(ty) = ty {
            builder.declare_var(Variable::from_u32(index as u32), *ty);
        }
    }
    for (block_index, mir_block) in body.basic_blocks.iter().enumerate() {
        let block = blocks[block_index];
        builder.switch_to_block(block);
        if block_index == 0 {
            let parameters = builder.block_params(entry).to_vec();
            let parameter_count = parameters.len();
            for (index, value) in parameters.into_iter().enumerate() {
                builder.def_var(Variable::from_u32((index + 1) as u32), value);
            }
            if context.runtime_free.is_some() {
                let zero = builder.ins().iconst(types::I64, 0);
                for index in (parameter_count + 1)..body.local_decls.len() {
                    if is_owned_string_local(body, index) {
                        builder.def_var(Variable::from_u32(index as u32), zero);
                    }
                }
                if is_owned_string_local(body, 0) {
                    builder.def_var(Variable::from_u32(0), zero);
                }
            }
            for (index, local) in body.local_decls.iter().enumerate() {
                let Some(symbol) = &local.global_symbol else {
                    continue;
                };
                let data_id = context
                    .global_ids
                    .get(symbol)
                    .copied()
                    .ok_or_else(|| format!("Globalvariable '{symbol}' fehlt im COFF-Modul."))?;
                let global = module.declare_data_in_func(data_id, builder.func);
                let address = builder.ins().symbol_value(types::I64, global);
                let ty = variable_types[index]
                    .ok_or_else(|| format!("Globalvariable '{symbol}' besitzt keinen Werttyp."))?;
                let value = builder.ins().load(ty, MemFlags::new(), address, 0);
                builder.def_var(Variable::from_u32(index as u32), value);
            }
        }
        for statement in &mir_block.statements {
            match &statement.kind {
                StatementKind::Assign(destination, rvalue) => {
                    if let Rvalue::Aggregate(fields) = rvalue {
                        let local = &body.local_decls[destination.local.0];
                        if !destination.projection.is_empty()
                            || !local.type_name.starts_with("class ")
                            || fields.len() != local.field_types.len()
                        {
                            return Err(
                                "Aggregate werden nur für Klassenobjekte unterstützt.".to_owned()
                            );
                        }
                        let address =
                            builder.use_var(Variable::from_u32(destination.local.0 as u32));
                        for (index, field) in fields.iter().enumerate() {
                            let ty = cranelift_type(&local.field_types[index])?;
                            let value = lower_operand(
                                module,
                                &mut builder,
                                field,
                                ty,
                                body,
                                context.string_constants,
                            )?;
                            builder.ins().store(
                                MemFlags::new(),
                                value,
                                address,
                                (index * 8) as i32,
                            );
                        }
                        continue;
                    }
                    if let [ProjectionElem::Field(field)] = destination.projection.as_slice() {
                        let ty = place_type(destination, body)?;
                        let value = lower_rvalue(
                            module,
                            &mut builder,
                            rvalue,
                            ty,
                            body,
                            context.string_constants,
                            context.overflow_checks,
                        )?;
                        let address =
                            builder.use_var(Variable::from_u32(destination.local.0 as u32));
                        builder.ins().trapz(address, TrapCode::unwrap_user(4));
                        builder
                            .ins()
                            .store(MemFlags::new(), value, address, (field.0 * 8) as i32);
                        continue;
                    }
                    if !destination.projection.is_empty() {
                        return Err("Feld- und Dereferenzierungszuweisungen sind im Codegen-Subset nicht unterstützt.".to_owned());
                    }
                    let value = if let Some(value) =
                        lower_string_binary(module, &mut builder, rvalue, context)?
                    {
                        value
                    } else {
                        lower_rvalue(
                            module,
                            &mut builder,
                            rvalue,
                            variable_types[destination.local.0].ok_or_else(|| {
                                "void kann nicht als Zuweisungsziel verwendet werden.".to_owned()
                            })?,
                            body,
                            context.string_constants,
                            context.overflow_checks,
                        )?
                    };
                    if context.runtime_free.is_some()
                        && is_owned_string_local(body, destination.local.0)
                    {
                        let source = moved_string_source(rvalue, body)?;
                        let old = builder.use_var(Variable::from_u32(destination.local.0 as u32));
                        if let Some(source) = source {
                            let zero = builder.ins().iconst(types::I64, 0);
                            builder.def_var(Variable::from_u32(source as u32), zero);
                        }
                        if source != Some(destination.local.0) {
                            emit_string_free(module, &mut builder, context, old);
                        }
                    }
                    builder.def_var(Variable::from_u32(destination.local.0 as u32), value);
                    if let Some(symbol) = &body.local_decls[destination.local.0].global_symbol {
                        let data_id = context.global_ids.get(symbol).copied().ok_or_else(|| {
                            format!("Globalvariable '{symbol}' fehlt im COFF-Modul.")
                        })?;
                        let global = module.declare_data_in_func(data_id, builder.func);
                        let address = builder.ins().symbol_value(types::I64, global);
                        if context.runtime_free.is_some()
                            && body.local_decls[destination.local.0].type_name == "string"
                        {
                            // Reload: another function may have replaced the cached value.
                            let old = builder.ins().load(types::I64, MemFlags::new(), address, 0);
                            emit_string_free(module, &mut builder, context, old);
                        }
                        builder.ins().store(MemFlags::new(), value, address, 0);
                    }
                }
                StatementKind::StorageDead(local) => {
                    if context.runtime_free.is_some() && is_owned_string_local(body, local.0) {
                        let old = builder.use_var(Variable::from_u32(local.0 as u32));
                        emit_string_free(module, &mut builder, context, old);
                        let zero = builder.ins().iconst(types::I64, 0);
                        builder.def_var(Variable::from_u32(local.0 as u32), zero);
                    }
                }
                StatementKind::StorageLive(_) | StatementKind::Nop => {}
                StatementKind::Deinit(_) => {
                    return Err("MIR-Deinit ist im Codegen-Subset nicht unterstützt.".to_owned());
                }
            }
        }

        let terminator = &mir_block
            .terminator
            .as_ref()
            .ok_or_else(|| format!("MIR-BB{block_index} besitzt keinen Terminator."))?
            .kind;
        match terminator {
            TerminatorKind::Goto { target } => {
                builder.ins().jump(blocks[target.0], &[]);
            }
            TerminatorKind::SwitchInt {
                discr,
                targets,
                otherwise,
            } => lower_switch(
                &mut builder,
                discr,
                targets,
                blocks[otherwise.0],
                &blocks,
                body,
            )?,
            TerminatorKind::Return => {
                if context.is_entry
                    && let Some(runtime_id) = context.runtime_object_shutdown
                {
                    let runtime_ref = module.declare_func_in_func(*runtime_id, builder.func);
                    builder.ins().call(runtime_ref, &[]);
                }
                if context.runtime_free.is_some() {
                    for index in 1..body.local_decls.len() {
                        if is_owned_string_local(body, index) {
                            let old = builder.use_var(Variable::from_u32(index as u32));
                            emit_string_free(module, &mut builder, context, old);
                        }
                    }
                }
                if body.local_decls[0].type_name == "void" {
                    builder.ins().return_(&[]);
                } else {
                    let result = builder.use_var(Variable::from_u32(LocalId(0).0 as u32));
                    builder.ins().return_(&[result]);
                }
            }
            TerminatorKind::Unreachable => {
                builder.ins().trap(TrapCode::unwrap_user(1));
            }
            TerminatorKind::Call {
                callee,
                args,
                destination,
                target,
                cleanup,
            } => {
                if cleanup.is_some() {
                    return Err(
                        "Aufruf mit Cleanup-Kante wird im Codegen-Subset nicht unterstützt."
                            .to_owned(),
                    );
                }
                if let Some(newline) = console_newline(callee) {
                    let runtime_id = context.runtime_console.ok_or_else(|| {
                        "Konsolenausgabe fehlt in der Runtime-Funktionsdeklaration.".to_owned()
                    })?;
                    let [argument] = args.as_slice() else {
                        return Err(
                            "Console.write/writeLine benötigt genau einen Stringwert.".to_owned()
                        );
                    };
                    if !operand_is_string(argument, body) {
                        return Err(
                            "Console.write/writeLine benötigt einen Wert vom Typ 'string'."
                                .to_owned(),
                        );
                    }
                    let descriptor = lower_operand(
                        module,
                        &mut builder,
                        argument,
                        types::I64,
                        body,
                        context.string_constants,
                    )?;
                    let pointer = builder
                        .ins()
                        .load(types::I64, MemFlags::new(), descriptor, 0);
                    let length = builder
                        .ins()
                        .load(types::I64, MemFlags::new(), descriptor, 8);
                    let newline = builder.ins().iconst(types::I8, newline);
                    let runtime_ref = module.declare_func_in_func(*runtime_id, builder.func);
                    let call = builder.ins().call(runtime_ref, &[pointer, length, newline]);
                    let status = builder.inst_results(call)[0];
                    let succeeded = builder.ins().icmp_imm(IntCC::Equal, status, 0);
                    let failure = builder.create_block();
                    builder
                        .ins()
                        .brif(succeeded, blocks[target.0], &[], failure, &[]);
                    builder.switch_to_block(failure);
                    builder.ins().trap(TrapCode::unwrap_user(2));
                    continue;
                }
                if let Some(builtin) = builtins::find(callee) {
                    let id = context
                        .runtime_builtins
                        .get(builtin.name)
                        .ok_or_else(|| format!("Runtime-Import für '{callee}' fehlt."))?;
                    if args.len() != builtin.params.len() {
                        return Err(format!(
                            "Aufruf von '{callee}' hat {} Argumente, erwartet werden {}.",
                            args.len(),
                            builtin.params.len()
                        ));
                    }
                    let mut values = Vec::new();
                    for (argument, expected) in args.iter().zip(builtin.params) {
                        let value_type = if *expected == "string" {
                            types::I64
                        } else {
                            cranelift_type(expected)?
                        };
                        let value = lower_operand(
                            module,
                            &mut builder,
                            argument,
                            value_type,
                            body,
                            context.string_constants,
                        )?;
                        if *expected == "string" {
                            values.push(builder.ins().load(types::I64, MemFlags::new(), value, 0));
                            values.push(builder.ins().load(types::I64, MemFlags::new(), value, 8));
                        } else {
                            values.push(value);
                        }
                    }
                    let function_ref = module.declare_func_in_func(*id, builder.func);
                    let call = builder.ins().call(function_ref, &values);
                    let result = builder.inst_results(call)[0];
                    if builtin.returns == "string" {
                        // Null meldet Speichermangel in der Runtime.
                        builder.ins().trapz(result, TrapCode::unwrap_user(3));
                        if is_owned_string_local(body, destination.local.0) {
                            let old =
                                builder.use_var(Variable::from_u32(destination.local.0 as u32));
                            emit_string_free(module, &mut builder, context, old);
                        }
                    }
                    builder.def_var(Variable::from_u32(destination.local.0 as u32), result);
                    builder.ins().jump(blocks[target.0], &[]);
                    continue;
                }
                if let Some(dispatch) = callee.strip_prefix("__nortex_virtual|") {
                    let mut parts = dispatch.split('|');
                    let method_name = parts.next().unwrap_or("<method>");
                    let targets = parts
                        .filter_map(|target| {
                            let (id, name) = target.split_once('=')?;
                            Some((id.parse::<i64>().ok()?, name))
                        })
                        .collect::<Vec<_>>();
                    let Some(receiver) = args.first() else {
                        return Err(format!(
                            "Virtueller Aufruf '{method_name}' besitzt keinen Receiver."
                        ));
                    };
                    let receiver = lower_operand(
                        module,
                        &mut builder,
                        receiver,
                        types::I64,
                        body,
                        context.string_constants,
                    )?;
                    builder.ins().trapz(receiver, TrapCode::unwrap_user(4));
                    let type_id = builder
                        .ins()
                        .load(types::I64, MemFlags::new(), receiver, -8);
                    for (class_id, target_name) in targets {
                        let Some(id) = context.function_ids.get(target_name).copied() else {
                            return Err(format!(
                                "Virtuelles Ziel '{target_name}' für '{method_name}' fehlt."
                            ));
                        };
                        let target_index =
                            *context.function_indices.get(target_name).ok_or_else(|| {
                                format!("Signatur für virtuelles Ziel '{target_name}' fehlt.")
                            })?;
                        let signature = &context.signatures[target_index];
                        if args.len() != signature.params.len() {
                            return Err(format!(
                                "Virtueller Aufruf '{method_name}' hat {} Argumente, erwartet werden {}.",
                                args.len(),
                                signature.params.len()
                            ));
                        }
                        let call_block = builder.create_block();
                        let next_block = builder.create_block();
                        let matches = builder.ins().icmp_imm(IntCC::Equal, type_id, class_id);
                        builder
                            .ins()
                            .brif(matches, call_block, &[], next_block, &[]);
                        builder.switch_to_block(call_block);
                        let mut values = Vec::with_capacity(args.len());
                        for (argument, parameter) in args.iter().zip(&signature.params) {
                            values.push(lower_operand(
                                module,
                                &mut builder,
                                argument,
                                parameter.value_type,
                                body,
                                context.string_constants,
                            )?);
                        }
                        let function_ref = module.declare_func_in_func(id, builder.func);
                        let call = builder.ins().call(function_ref, &values);
                        if let Some(result) = builder.inst_results(call).first() {
                            builder
                                .def_var(Variable::from_u32(destination.local.0 as u32), *result);
                        }
                        builder.ins().jump(blocks[target.0], &[]);
                        builder.switch_to_block(next_block);
                    }
                    builder.ins().trap(TrapCode::unwrap_user(5));
                    continue;
                }
                let id = context
                    .function_ids
                    .get(callee)
                    .copied()
                    .ok_or_else(|| format!("Unbekannte MIR-Funktion '{callee}'."))?;
                let callee_index = *context
                    .function_indices
                    .get(callee)
                    .ok_or_else(|| format!("Signatur für MIR-Funktion '{callee}' fehlt."))?;
                let signature = &context.signatures[callee_index];
                if args.len() != signature.params.len() {
                    return Err(format!(
                        "Aufruf von '{callee}' hat {} Argumente, erwartet werden {}.",
                        args.len(),
                        signature.params.len()
                    ));
                }
                let mut values = Vec::with_capacity(args.len());
                let is_external = context.external_functions.contains(callee);
                let mut moved_sources = Vec::new();
                for (argument, parameter) in args.iter().zip(&signature.params) {
                    if !is_external
                        && context.runtime_free.is_some()
                        && let Some(source) = moved_operand_source(argument, body)?
                    {
                        moved_sources.push(source);
                    }
                    let value = lower_operand(
                        module,
                        &mut builder,
                        argument,
                        parameter.value_type,
                        body,
                        context.string_constants,
                    )?;
                    // Strings sind intern NUL-terminiert; C erhält den Datenzeiger (char*).
                    values.push(if is_external && operand_is_string(argument, body) {
                        builder.ins().load(types::I64, MemFlags::new(), value, 0)
                    } else {
                        value
                    });
                }
                let function_ref = module.declare_func_in_func(id, builder.func);
                let call = builder.ins().call(function_ref, &values);
                let results = builder.inst_results(call).to_vec();
                if callee == "nortex_object_alloc"
                    && let Some(result) = results.first()
                {
                    builder.ins().trapz(*result, TrapCode::unwrap_user(6));
                }
                let zero = builder.ins().iconst(types::I64, 0);
                for source in moved_sources {
                    builder.def_var(Variable::from_u32(source as u32), zero);
                }
                if let Some(result) = results.first() {
                    if context.runtime_free.is_some()
                        && is_owned_string_local(body, destination.local.0)
                    {
                        let old = builder.use_var(Variable::from_u32(destination.local.0 as u32));
                        emit_string_free(module, &mut builder, context, old);
                    }
                    builder.def_var(Variable::from_u32(destination.local.0 as u32), *result);
                }
                builder.ins().jump(blocks[target.0], &[]);
            }
            TerminatorKind::Drop { .. } => {
                return Err("MIR-Drop ist im Codegen-Subset nicht unterstützt.".to_owned());
            }
            TerminatorKind::Assert {
                cond,
                expected,
                target,
                cleanup,
                ..
            } => {
                let condition = lower_operand(
                    module,
                    &mut builder,
                    cond,
                    types::I8,
                    body,
                    context.string_constants,
                )?;
                let condition =
                    builder
                        .ins()
                        .icmp_imm(IntCC::Equal, condition, i64::from(*expected));
                let failure =
                    cleanup.map_or_else(|| builder.create_block(), |cleanup| blocks[cleanup.0]);
                builder
                    .ins()
                    .brif(condition, blocks[target.0], &[], failure, &[]);
                if cleanup.is_none() {
                    builder.switch_to_block(failure);
                    builder.ins().trap(TrapCode::unwrap_user(1));
                }
            }
        }
    }

    builder.seal_all_blocks();
    builder.finalize();
    Ok(())
}

fn uses_console_output(functions: &[LoweredMirFunction]) -> bool {
    functions.iter().any(|function| {
        function.body.basic_blocks.iter().any(|block| {
            matches!(
                block.terminator.as_ref().map(|terminator| &terminator.kind),
                Some(TerminatorKind::Call { callee, .. })
                    if console_newline(callee).is_some()
            )
        })
    })
}

fn uses_object_allocation(functions: &[LoweredMirFunction]) -> bool {
    functions.iter().any(|function| {
        function.body.basic_blocks.iter().any(|block| {
            matches!(
                block.terminator.as_ref().map(|terminator| &terminator.kind),
                Some(TerminatorKind::Call { callee, .. }) if callee == "nortex_object_alloc"
            )
        })
    })
}

fn used_builtins(functions: &[LoweredMirFunction]) -> Vec<&'static Builtin> {
    let mut used = Vec::<&'static Builtin>::new();
    for function in functions {
        for block in &function.body.basic_blocks {
            if let Some(TerminatorKind::Call { callee, .. }) =
                block.terminator.as_ref().map(|terminator| &terminator.kind)
                && let Some(builtin) = builtins::find(callee)
                && !used.iter().any(|known| known.name == builtin.name)
            {
                used.push(builtin);
            }
        }
    }
    used
}

fn declare_builtin_imports(
    module: &mut ObjectModule,
    functions: &[LoweredMirFunction],
) -> Result<HashMap<&'static str, FuncId>, String> {
    let mut ids = HashMap::new();
    for builtin in used_builtins(functions) {
        let mut signature = module.make_signature();
        for parameter in builtin.params {
            if *parameter == "string" {
                signature.params.extend([AbiParam::new(types::I64); 2]);
            } else {
                signature
                    .params
                    .push(AbiParam::new(cranelift_type(parameter)?));
            }
        }
        signature
            .returns
            .push(AbiParam::new(cranelift_type(builtin.returns)?));
        let id = module
            .declare_function(builtin.symbol, Linkage::Import, &signature)
            .map_err(|error| {
                format!(
                    "Runtime-Funktion '{}' konnte nicht deklariert werden: {error}",
                    builtin.name
                )
            })?;
        ids.insert(builtin.name, id);
    }
    Ok(ids)
}

fn uses_string_eq(functions: &[LoweredMirFunction]) -> bool {
    functions.iter().any(|function| {
        function.body.basic_blocks.iter().any(|block| {
            block.statements.iter().any(|statement| {
                matches!(
                    &statement.kind,
                    StatementKind::Assign(_, Rvalue::BinaryOp(BinaryOp::Eq | BinaryOp::Ne, left, _))
                        if operand_is_string(left, &function.body)
                )
            })
        })
    })
}

fn uses_string_concat(functions: &[LoweredMirFunction]) -> bool {
    functions.iter().any(|function| {
        function.body.basic_blocks.iter().any(|block| {
            block.statements.iter().any(|statement| {
                matches!(
                    &statement.kind,
                    StatementKind::Assign(_, Rvalue::BinaryOp(BinaryOp::Add, left, _))
                        if operand_is_string(left, &function.body)
                )
            })
        })
    })
}

fn is_owned_string_local(body: &MirBody, index: usize) -> bool {
    let local = &body.local_decls[index];
    local.type_name == "string" && local.global_symbol.is_none()
}

/// Returns the local whose heap string is transferred by `operand` and must be zeroed.
fn moved_operand_source(operand: &Operand, body: &MirBody) -> Result<Option<usize>, String> {
    let (Operand::Copy(place) | Operand::Move(place)) = operand else {
        return Ok(None);
    };
    if !place.projection.is_empty() || !is_owned_string_local(body, place.local.0) {
        return Ok(None);
    }
    let local = &body.local_decls[place.local.0];
    if matches!(operand, Operand::Copy(_)) && local.is_user_variable && place.local.0 != 0 {
        return Err(format!(
            "String '{}' kann hier nicht kopiert werden; Strings besitzen ihren Speicher und werden verschoben.",
            local.name.as_deref().unwrap_or("?")
        ));
    }
    Ok(Some(place.local.0))
}

fn moved_string_source(rvalue: &Rvalue, body: &MirBody) -> Result<Option<usize>, String> {
    match rvalue {
        Rvalue::Use(operand) => moved_operand_source(operand, body),
        _ => Ok(None),
    }
}

fn emit_string_free(
    module: &mut ObjectModule,
    builder: &mut FunctionBuilder,
    context: &FunctionLoweringContext<'_>,
    descriptor: cranelift_codegen::ir::Value,
) {
    if let Some(id) = context.runtime_free {
        let function_ref = module.declare_func_in_func(*id, builder.func);
        builder.ins().call(function_ref, &[descriptor]);
    }
}

/// Lowers `string + string`, `string == string` and `string != string`.
fn lower_string_binary(
    module: &mut ObjectModule,
    builder: &mut FunctionBuilder,
    rvalue: &Rvalue,
    context: &FunctionLoweringContext<'_>,
) -> Result<Option<cranelift_codegen::ir::Value>, String> {
    let Rvalue::BinaryOp(operator, left, right) = rvalue else {
        return Ok(None);
    };
    let body = context.body;
    if !operand_is_string(left, body) {
        return Ok(None);
    }
    match operator {
        BinaryOp::Add => {
            let runtime_id = context.runtime_concat.copied().ok_or_else(|| {
                "String-Konkatenation fehlt in der Runtime-Funktionsdeklaration.".to_owned()
            })?;
            lower_string_concat(
                module,
                builder,
                (left, right),
                body,
                context.string_constants,
                runtime_id,
            )
            .map(Some)
        }
        BinaryOp::Eq | BinaryOp::Ne => {
            if !operand_is_string(right, body) {
                return Err("Stringvergleiche benötigen zwei Werte vom Typ 'string'.".to_owned());
            }
            let runtime_id = context.runtime_eq.copied().ok_or_else(|| {
                "String-Vergleich fehlt in der Runtime-Funktionsdeklaration.".to_owned()
            })?;
            let mut arguments = Vec::with_capacity(4);
            for operand in [left, right] {
                let descriptor = lower_operand(
                    module,
                    builder,
                    operand,
                    types::I64,
                    body,
                    context.string_constants,
                )?;
                arguments.push(
                    builder
                        .ins()
                        .load(types::I64, MemFlags::new(), descriptor, 0),
                );
                arguments.push(
                    builder
                        .ins()
                        .load(types::I64, MemFlags::new(), descriptor, 8),
                );
            }
            let runtime_ref = module.declare_func_in_func(runtime_id, builder.func);
            let call = builder.ins().call(runtime_ref, &arguments);
            let result = builder.inst_results(call)[0];
            Ok(Some(if matches!(operator, BinaryOp::Ne) {
                builder.ins().icmp_imm(IntCC::Equal, result, 0)
            } else {
                result
            }))
        }
        _ => Ok(None),
    }
}

fn lower_string_concat(
    module: &mut ObjectModule,
    builder: &mut FunctionBuilder,
    operands: (&Operand, &Operand),
    body: &MirBody,
    string_constants: &HashMap<String, DataId>,
    runtime_id: FuncId,
) -> Result<cranelift_codegen::ir::Value, String> {
    let (left, right) = operands;
    if !operand_is_string(left, body) || !operand_is_string(right, body) {
        return Err("Der Operator '+' verknüpft nur zwei Werte vom Typ 'string'.".to_owned());
    }
    let mut arguments = Vec::with_capacity(4);
    for operand in [left, right] {
        let descriptor =
            lower_operand(module, builder, operand, types::I64, body, string_constants)?;
        arguments.push(
            builder
                .ins()
                .load(types::I64, MemFlags::new(), descriptor, 0),
        );
        arguments.push(
            builder
                .ins()
                .load(types::I64, MemFlags::new(), descriptor, 8),
        );
    }
    let runtime_ref = module.declare_func_in_func(runtime_id, builder.func);
    let call = builder.ins().call(runtime_ref, &arguments);
    let result = builder.inst_results(call)[0];
    let succeeded = builder.ins().icmp_imm(IntCC::NotEqual, result, 0);
    let failure = builder.create_block();
    let continuation = builder.create_block();
    builder
        .ins()
        .brif(succeeded, continuation, &[], failure, &[]);
    builder.switch_to_block(failure);
    builder.ins().trap(TrapCode::unwrap_user(3));
    builder.switch_to_block(continuation);
    Ok(result)
}

fn declare_string_constants(
    module: &mut ObjectModule,
    functions: &[LoweredMirFunction],
    symbols: &mut HashSet<String>,
) -> Result<HashMap<String, DataId>, String> {
    let mut constants = BTreeSet::new();
    for function in functions {
        for block in &function.body.basic_blocks {
            for statement in &block.statements {
                if let StatementKind::Assign(_, value) = &statement.kind {
                    match value {
                        Rvalue::Use(operand) | Rvalue::Cast(operand, _) => {
                            collect_string_constant(operand, &mut constants);
                        }
                        Rvalue::BinaryOp(_, left, right) => {
                            collect_string_constant(left, &mut constants);
                            collect_string_constant(right, &mut constants);
                        }
                        Rvalue::Aggregate(operands) => {
                            for operand in operands {
                                collect_string_constant(operand, &mut constants);
                            }
                        }
                        Rvalue::Ref(_, _) | Rvalue::Discriminant(_) => {}
                    }
                }
            }
            match block.terminator.as_ref().map(|terminator| &terminator.kind) {
                Some(TerminatorKind::Call { args, .. }) => {
                    for argument in args {
                        collect_string_constant(argument, &mut constants);
                    }
                }
                Some(TerminatorKind::SwitchInt { discr, .. }) => {
                    collect_string_constant(discr, &mut constants);
                }
                Some(TerminatorKind::Assert { cond, .. }) => {
                    collect_string_constant(cond, &mut constants);
                }
                _ => {}
            }
        }
    }

    let mut string_constants = HashMap::with_capacity(constants.len());
    for (index, text) in constants.into_iter().enumerate() {
        let slice_id = define_string_descriptor(module, symbols, &text, index)?;
        string_constants.insert(text, slice_id);
    }
    Ok(string_constants)
}

/// Defines NUL-terminated UTF-8 bytes plus a 16-byte `{ptr, len}` descriptor in `.rdata`.
fn define_string_descriptor(
    module: &mut ObjectModule,
    symbols: &mut HashSet<String>,
    text: &str,
    start_index: usize,
) -> Result<DataId, String> {
    {
        let length = u64::try_from(text.len())
            .map_err(|_| "Stringliteral überschreitet die native Slice-Länge.".to_owned())?;
        let mut bytes = text.as_bytes().to_vec();
        bytes.push(0);

        let mut symbol_index = start_index;
        let (bytes_symbol, slice_symbol) = loop {
            let bytes_symbol = format!("nortex_string_bytes_{symbol_index}");
            let slice_symbol = format!("nortex_string_slice_{symbol_index}");
            if !symbols.contains(&bytes_symbol) && !symbols.contains(&slice_symbol) {
                symbols.insert(bytes_symbol.clone());
                symbols.insert(slice_symbol.clone());
                break (bytes_symbol, slice_symbol);
            }
            symbol_index += 1;
        };

        let bytes_id = module
            .declare_data(&bytes_symbol, Linkage::Local, false, false)
            .map_err(|error| {
                format!("UTF-8-Stringdaten konnten nicht deklariert werden: {error}")
            })?;
        let mut byte_data = DataDescription::new();
        byte_data.set_align(1);
        byte_data.define(bytes.into_boxed_slice());
        module.define_data(bytes_id, &byte_data).map_err(|error| {
            format!("UTF-8-Stringdaten konnten nicht gespeichert werden: {error}")
        })?;

        let slice_id = module
            .declare_data(&slice_symbol, Linkage::Local, false, false)
            .map_err(|error| format!("String-Slice konnte nicht deklariert werden: {error}"))?;
        let mut slice_data = DataDescription::new();
        slice_data.set_align(16);
        let byte_address = module.declare_data_in_data(bytes_id, &mut slice_data);
        slice_data.write_data_addr(16, byte_address, 0);
        // 16 zero bytes precede the descriptor so `nortex_string_free` ignores static data.
        let mut descriptor = vec![0; 32];
        descriptor[24..].copy_from_slice(&length.to_le_bytes());
        slice_data.define(descriptor.into_boxed_slice());
        module
            .define_data(slice_id, &slice_data)
            .map_err(|error| format!("String-Slice konnte nicht gespeichert werden: {error}"))?;
        Ok(slice_id)
    }
}

fn collect_string_constant(operand: &Operand, constants: &mut BTreeSet<String>) {
    if let Operand::ConstantString(text) = operand {
        constants.insert(text.clone());
    }
}

fn console_newline(callee: &str) -> Option<i64> {
    match callee {
        "__nortex_console_write" => Some(0),
        "__nortex_console_write_line" => Some(1),
        _ => None,
    }
}

fn lower_switch(
    builder: &mut FunctionBuilder,
    discr: &Operand,
    targets: &[(i64, crate::mir::BasicBlockId)],
    otherwise: Block,
    blocks: &[Block],
    body: &MirBody,
) -> Result<(), String> {
    let discr_type = operand_type(discr, body)?;
    if operand_is_string(discr, body) {
        return Err("Stringwerte werden im Switch-Codegen nicht unterstützt.".to_owned());
    }
    let value = match discr {
        Operand::Copy(place) | Operand::Move(place) if place.projection.is_empty() => {
            let actual_type = cranelift_type(&body.local_decls[place.local.0].type_name)?;
            if actual_type != discr_type {
                return Err(format!(
                    "Switch-Discriminant hat Cranelift-Typ {actual_type}, erwartet wird {discr_type}."
                ));
            }
            builder.use_var(Variable::from_u32(place.local.0 as u32))
        }
        Operand::ConstantInt(value) => builder.ins().iconst(discr_type, *value),
        Operand::ConstantBool(value) => builder.ins().iconst(types::I8, i64::from(*value)),
        _ => {
            return Err(
                "Switch-Discriminant besitzt einen nicht unterstützten Operanden.".to_owned(),
            );
        }
    };
    for (index, (case, target)) in targets.iter().enumerate() {
        let next = if index + 1 == targets.len() {
            otherwise
        } else {
            builder.create_block()
        };
        let condition = builder.ins().icmp_imm(IntCC::Equal, value, *case);
        builder
            .ins()
            .brif(condition, blocks[target.0], &[], next, &[]);
        if next != otherwise {
            builder.switch_to_block(next);
        }
    }
    if targets.is_empty() {
        builder.ins().jump(otherwise, &[]);
    }
    Ok(())
}

/// Emits a signed overflow-checked operation and traps (user code 3) on overflow.
fn trap_on_overflow(
    builder: &mut FunctionBuilder,
    operator: BinaryOp,
    left: cranelift_codegen::ir::Value,
    right: cranelift_codegen::ir::Value,
) -> cranelift_codegen::ir::Value {
    let (value, overflow) = match operator {
        BinaryOp::Add => builder.ins().sadd_overflow(left, right),
        BinaryOp::Sub => builder.ins().ssub_overflow(left, right),
        _ => builder.ins().smul_overflow(left, right),
    };
    builder.ins().trapnz(overflow, TrapCode::unwrap_user(3));
    value
}

fn lower_float_binary(
    builder: &mut FunctionBuilder,
    operator: BinaryOp,
    left: cranelift_codegen::ir::Value,
    right: cranelift_codegen::ir::Value,
) -> Result<cranelift_codegen::ir::Value, String> {
    let compare =
        |builder: &mut FunctionBuilder, condition| builder.ins().fcmp(condition, left, right);
    Ok(match operator {
        BinaryOp::Add => builder.ins().fadd(left, right),
        BinaryOp::Sub => builder.ins().fsub(left, right),
        BinaryOp::Mul => builder.ins().fmul(left, right),
        BinaryOp::Div => builder.ins().fdiv(left, right),
        BinaryOp::Eq => compare(builder, FloatCC::Equal),
        BinaryOp::Ne => compare(builder, FloatCC::NotEqual),
        BinaryOp::Lt => compare(builder, FloatCC::LessThan),
        BinaryOp::Le => compare(builder, FloatCC::LessThanOrEqual),
        BinaryOp::Gt => compare(builder, FloatCC::GreaterThan),
        BinaryOp::Ge => compare(builder, FloatCC::GreaterThanOrEqual),
        other => {
            return Err(format!(
                "Der Operator {other:?} ist für double nicht unterstützt."
            ));
        }
    })
}

fn lower_rvalue(
    module: &mut ObjectModule,
    builder: &mut FunctionBuilder,
    value: &Rvalue,
    destination_type: cranelift_codegen::ir::Type,
    body: &MirBody,
    string_constants: &HashMap<String, DataId>,
    overflow_checks: bool,
) -> Result<cranelift_codegen::ir::Value, String> {
    match value {
        Rvalue::Use(operand) => lower_operand(
            module,
            builder,
            operand,
            destination_type,
            body,
            string_constants,
        ),
        Rvalue::BinaryOp(operator, left, right) => {
            if operand_is_string(left, body) || operand_is_string(right, body) {
                return Err(
                    "Stringoperatoren sind im nativen Codegen-Subset nicht unterstützt.".to_owned(),
                );
            }
            let operand_type = match (operand_type(left, body), operand_type(right, body)) {
                (Ok(types::F64), _) | (_, Ok(types::F64)) => types::F64,
                (Ok(left_type), _) => left_type,
                (_, Ok(right_type)) => right_type,
                _ => destination_type,
            };
            let unsigned = operand_unsigned(left, body) || operand_unsigned(right, body);
            let left = lower_operand(module, builder, left, operand_type, body, string_constants)?;
            let right =
                lower_operand(module, builder, right, operand_type, body, string_constants)?;
            if operand_type == types::F64 {
                return lower_float_binary(builder, *operator, left, right);
            }
            let checked = overflow_checks
                && !unsigned
                && (operand_type == types::I32 || operand_type == types::I64);
            Ok(match operator {
                BinaryOp::Add if checked => trap_on_overflow(builder, *operator, left, right),
                BinaryOp::Sub if checked => trap_on_overflow(builder, *operator, left, right),
                BinaryOp::Mul if checked => trap_on_overflow(builder, *operator, left, right),
                BinaryOp::Add => builder.ins().iadd(left, right),
                BinaryOp::Sub => builder.ins().isub(left, right),
                BinaryOp::Mul => builder.ins().imul(left, right),
                BinaryOp::Div if unsigned => builder.ins().udiv(left, right),
                BinaryOp::Div => builder.ins().sdiv(left, right),
                BinaryOp::Rem if unsigned => builder.ins().urem(left, right),
                BinaryOp::Rem => builder.ins().srem(left, right),
                BinaryOp::BitXor => builder.ins().bxor(left, right),
                BinaryOp::BitAnd => builder.ins().band(left, right),
                BinaryOp::BitOr => builder.ins().bor(left, right),
                BinaryOp::Shl => builder.ins().ishl(left, right),
                BinaryOp::Shr if unsigned => builder.ins().ushr(left, right),
                BinaryOp::Shr => builder.ins().sshr(left, right),
                BinaryOp::Eq => builder.ins().icmp(IntCC::Equal, left, right),
                BinaryOp::Ne => builder.ins().icmp(IntCC::NotEqual, left, right),
                BinaryOp::Lt if unsigned => {
                    builder.ins().icmp(IntCC::UnsignedLessThan, left, right)
                }
                BinaryOp::Lt => builder.ins().icmp(IntCC::SignedLessThan, left, right),
                BinaryOp::Le if unsigned => {
                    builder
                        .ins()
                        .icmp(IntCC::UnsignedLessThanOrEqual, left, right)
                }
                BinaryOp::Le => builder
                    .ins()
                    .icmp(IntCC::SignedLessThanOrEqual, left, right),
                BinaryOp::Gt if unsigned => {
                    builder.ins().icmp(IntCC::UnsignedGreaterThan, left, right)
                }
                BinaryOp::Gt => builder.ins().icmp(IntCC::SignedGreaterThan, left, right),
                BinaryOp::Ge if unsigned => {
                    builder
                        .ins()
                        .icmp(IntCC::UnsignedGreaterThanOrEqual, left, right)
                }
                BinaryOp::Ge => builder
                    .ins()
                    .icmp(IntCC::SignedGreaterThanOrEqual, left, right),
            })
        }
        Rvalue::Cast(operand, target) => {
            let source_type = operand_type(operand, body)?;
            let target_type = cranelift_type(target)?;
            let value = lower_operand(
                module,
                builder,
                operand,
                source_type,
                body,
                string_constants,
            )?;
            if source_type == target_type {
                Ok(value)
            } else if source_type.bits() > target_type.bits() {
                Ok(builder.ins().ireduce(target_type, value))
            } else if is_unsigned_type(target) {
                Ok(builder.ins().uextend(target_type, value))
            } else {
                Ok(builder.ins().sextend(target_type, value))
            }
        }
        Rvalue::Ref(_, _) | Rvalue::Aggregate(_) | Rvalue::Discriminant(_) => Err(
            "Referenzen, Aggregate und Discriminant sind im Codegen-Subset nicht unterstützt."
                .to_owned(),
        ),
    }
}

fn lower_operand(
    module: &mut ObjectModule,
    builder: &mut FunctionBuilder,
    operand: &Operand,
    expected_type: cranelift_codegen::ir::Type,
    body: &MirBody,
    string_constants: &HashMap<String, DataId>,
) -> Result<cranelift_codegen::ir::Value, String> {
    match operand {
        Operand::Copy(place) | Operand::Move(place) if place.projection.is_empty() => {
            let actual_type = cranelift_type(&body.local_decls[place.local.0].type_name)?;
            if actual_type != expected_type {
                return Err(format!(
                    "Operand _L{} hat Cranelift-Typ {actual_type}, erwartet wird {expected_type}.",
                    place.local.0
                ));
            }
            Ok(builder.use_var(Variable::from_u32(place.local.0 as u32)))
        }
        Operand::Copy(place) | Operand::Move(place) => {
            let [ProjectionElem::Field(field)] = place.projection.as_slice() else {
                return Err(
                    "Dereferenzierungsoperanden sind im Codegen-Subset nicht unterstützt."
                        .to_owned(),
                );
            };
            let actual_type = place_type(place, body)?;
            if actual_type != expected_type {
                return Err(format!(
                    "Feld _L{}.{} hat Cranelift-Typ {actual_type}, erwartet wird {expected_type}.",
                    place.local.0, field.0
                ));
            }
            let address = builder.use_var(Variable::from_u32(place.local.0 as u32));
            builder.ins().trapz(address, TrapCode::unwrap_user(4));
            Ok(builder
                .ins()
                .load(actual_type, MemFlags::new(), address, (field.0 * 8) as i32))
        }
        Operand::ConstantInt(value) if expected_type == types::F64 => {
            Ok(builder.ins().f64const(*value as f64))
        }
        Operand::ConstantInt(value) => Ok(builder.ins().iconst(expected_type, *value)),
        Operand::ConstantFloat(value) if expected_type == types::F64 => {
            Ok(builder.ins().f64const(*value))
        }
        Operand::ConstantFloat(_) => {
            Err("double-Konstanten benötigen einen double-Typ.".to_owned())
        }
        Operand::ConstantBool(value) => Ok(builder.ins().iconst(types::I8, i64::from(*value))),
        Operand::ConstantString(text) if expected_type == types::I64 => {
            let data_id = string_constants
                .get(text)
                .copied()
                .ok_or_else(|| format!("Stringliteral '{text}' fehlt im COFF-Datenpool."))?;
            let global = module.declare_data_in_func(data_id, builder.func);
            let base = builder.ins().symbol_value(types::I64, global);
            Ok(builder.ins().iadd_imm(base, 16))
        }
        Operand::ConstantString(_) => {
            Err("String-Konstanten benötigen einen i64-Zieltyp im Codegen.".to_owned())
        }
    }
}

fn operand_type(operand: &Operand, body: &MirBody) -> Result<cranelift_codegen::ir::Type, String> {
    match operand {
        Operand::Copy(place) | Operand::Move(place) if place.projection.is_empty() => {
            cranelift_type(&body.local_decls[place.local.0].type_name)
        }
        Operand::ConstantBool(_) => Ok(types::I8),
        Operand::ConstantInt(_) => Ok(types::I32),
        Operand::Copy(place) | Operand::Move(place) => place_type(place, body),
        Operand::ConstantString(_) => Ok(types::I64),
        Operand::ConstantFloat(_) => Ok(types::F64),
    }
}

fn place_type(place: &Place, body: &MirBody) -> Result<cranelift_codegen::ir::Type, String> {
    let local = &body.local_decls[place.local.0];
    match place.projection.as_slice() {
        [] => cranelift_type(&local.type_name),
        [ProjectionElem::Field(field)] => {
            let field_type = local
                .field_types
                .get(field.0)
                .ok_or_else(|| format!("Feld {} fehlt in _L{}.", field.0, place.local.0))?;
            cranelift_type(field_type)
        }
        _ => {
            Err("Dereferenzierungsoperanden besitzen im Codegen-Subset keinen Wertetyp.".to_owned())
        }
    }
}

fn operand_is_string(operand: &Operand, body: &MirBody) -> bool {
    match operand {
        Operand::Copy(place) | Operand::Move(place) if place.projection.is_empty() => {
            body.local_decls[place.local.0].type_name == "string"
        }
        Operand::ConstantString(_) => true,
        _ => false,
    }
}

fn cranelift_type(type_name: &str) -> Result<cranelift_codegen::ir::Type, String> {
    Ok(match type_name {
        "bool" => types::I8,
        "byte" | "sbyte" | "u8" | "i8" => types::I8,
        "short" | "ushort" | "u16" | "i16" => types::I16,
        "int" | "uint" | "u32" | "i32" => types::I32,
        "long" | "ulong" | "nint" | "nuint" | "isize" | "usize" | "u64" | "i64" => types::I64,
        "string" | "IntList" | "StringList" => types::I64,
        object if object.starts_with("class ") => types::I64,
        "double" => types::F64,
        "void" => return Err("void besitzt keinen Cranelift-Werttyp.".to_owned()),
        other => {
            return Err(format!(
                "MIR-Typ '{other}' wird im Codegen-Subset nicht unterstützt."
            ));
        }
    })
}

fn is_unsigned_type(type_name: &str) -> bool {
    matches!(
        type_name,
        "byte" | "ushort" | "uint" | "ulong" | "nuint" | "usize" | "u8" | "u16" | "u32" | "u64"
    )
}

fn operand_unsigned(operand: &Operand, body: &MirBody) -> bool {
    match operand {
        Operand::Copy(place) | Operand::Move(place) if place.projection.is_empty() => {
            is_unsigned_type(&body.local_decls[place.local.0].type_name)
        }
        Operand::Copy(place) | Operand::Move(place) => match place.projection.as_slice() {
            [ProjectionElem::Field(field)] => body.local_decls[place.local.0]
                .field_types
                .get(field.0)
                .is_some_and(|ty| is_unsigned_type(ty)),
            _ => false,
        },
        _ => false,
    }
}

fn external_symbol_name(name: &str) -> String {
    if name == "main" || name.ends_with(".main") {
        "main".to_owned()
    } else {
        format!("nortex_{}", name.replace('.', "_"))
    }
}

fn global_symbol_name(name: &str) -> String {
    format!("nortex_global_{}", name.replace('.', "_"))
}

fn global_bytes(global: &crate::mir::MirGlobal) -> Result<Vec<u8>, String> {
    let width = cranelift_type(&global.type_name)?.bytes() as usize;
    match (&global.value, global.type_name.as_str()) {
        (crate::mir::MirGlobalValue::Boolean(value), "bool") => Ok(vec![u8::from(*value)]),
        (crate::mir::MirGlobalValue::Integer(_), "bool") => Err(format!(
            "Globalvariable '{}' besitzt einen int-Wert, aber den Typ bool.",
            global.name
        )),
        (crate::mir::MirGlobalValue::Integer(value), _) => {
            let bytes = value.to_le_bytes();
            Ok(bytes[..width].to_vec())
        }
        (crate::mir::MirGlobalValue::String(_), _) => Err(format!(
            "Globalvariable '{}' besitzt einen string-Wert ohne Deskriptor.",
            global.name
        )),
        (crate::mir::MirGlobalValue::Boolean(_), _) => Err(format!(
            "Globalvariable '{}' besitzt einen bool-Wert, aber einen numerischen Typ.",
            global.name
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emits_windows_x64_coff_for_valid_scalar_mir() {
        let compiled = crate::driver::compile_source_to_mir("public int main() { return 40 + 2; }");
        assert!(
            !compiled.has_errors(),
            "frontend diagnostics: {:#?}",
            compiled.diagnostics
        );
        let object = CodeGenerator::new(Target::X86_64Windows)
            .emit_object(&compiled.functions)
            .expect("scalar MIR should emit a COFF object");

        assert_eq!(&object[..2], &[0x64, 0x86], "COFF machine must be AMD64");
        assert!(
            object.windows(4).any(|window| window == b"main"),
            "the COFF symbol table should contain the CRT entry symbol"
        );
    }

    #[test]
    fn rejects_reference_types_not_supported_by_native_codegen() {
        let compiled =
            crate::driver::compile_source_to_mir("public int calculate(&int value) { return 1; }");
        assert!(!compiled.has_errors(), "{:#?}", compiled.diagnostics);
        let error = CodeGenerator::new(Target::X86_64Windows)
            .emit_object(&compiled.functions)
            .expect_err("reference parameters need pointer codegen");
        assert!(error.contains("MIR-Typ '&int'"), "{error}");
    }

    #[test]
    fn rejects_unsupported_string_operators_but_accepts_concatenation() {
        let compiled = crate::driver::compile_source_to_mir(
            "public bool same(string left, string right) { return left < right; }",
        );
        assert!(!compiled.has_errors(), "{:#?}", compiled.diagnostics);
        let error = CodeGenerator::new(Target::X86_64Windows)
            .emit_object(&compiled.functions)
            .expect_err("string ordering must not become pointer comparison");
        assert!(error.contains("Stringoperatoren"), "{error}");

        let compiled = crate::driver::compile_source_to_mir(
            "public string join(string left, string right) { return left + right; }",
        );
        assert!(!compiled.has_errors(), "{:#?}", compiled.diagnostics);
        let generator = CodeGenerator::new(Target::X86_64Windows);
        generator
            .emit_object(&compiled.functions)
            .expect("string concatenation lowers to a runtime call");
        assert!(CodeGenerator::requires_runtime(&compiled.functions));
    }
}
