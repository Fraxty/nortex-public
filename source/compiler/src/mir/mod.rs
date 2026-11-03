//! NORTEX MIR, CFG validation, definite assignment, and conservative loan checks.
//!
//! Ownership validation tracks loans, reborrows, and declared field projections
//! using CFG local liveness. This is a conservative NLL foundation; complete
//! lifetime inference, region constraints, and reference-return metadata are
//! not represented yet.

pub mod lowering;

use std::collections::{HashSet, VecDeque};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BasicBlockId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LocalId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FieldIndex(pub usize);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ProjectionElem {
    Deref,
    Field(FieldIndex),
    Index(LocalId),
    Subslice { from: u64, to: u64 },
}

fn is_ancestor(ancestor: LocalId, mut reference: LocalId, loans: &HashSet<ActiveLoan>) -> bool {
    while let Some(parent) = loans
        .iter()
        .find(|loan| loan.reference == reference)
        .and_then(|loan| loan.parent)
    {
        if parent == ancestor {
            return true;
        }
        reference = parent;
    }
    false
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Place {
    pub local: LocalId,
    pub projection: Vec<ProjectionElem>,
}

impl Place {
    pub fn from_local(local: LocalId) -> Self {
        Self {
            local,
            projection: Vec::new(),
        }
    }

    pub fn project_field(mut self, field: FieldIndex) -> Self {
        self.projection.push(ProjectionElem::Field(field));
        self
    }

    pub fn project_deref(mut self) -> Self {
        self.projection.push(ProjectionElem::Deref);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BorrowKind {
    Shared,  // &T
    Shallow, // &T (nur Diskriminante)
    Mut,     // &mut T (exklusiv)
}

#[derive(Debug, Clone, PartialEq)]
pub enum Operand {
    Copy(Place),
    Move(Place),
    ConstantInt(i64),
    ConstantFloat(f64),
    ConstantBool(bool),
    ConstantString(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    BitXor,
    BitAnd,
    BitOr,
    Shl,
    Shr,
    Eq,
    Lt,
    Le,
    Ne,
    Ge,
    Gt,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Rvalue {
    Use(Operand),
    Ref(BorrowKind, Place),
    BinaryOp(BinaryOp, Operand, Operand),
    Cast(Operand, String),
    Aggregate(Vec<Operand>),
    Discriminant(Place),
}

#[derive(Debug, Clone, PartialEq)]
pub enum StatementKind {
    Assign(Place, Rvalue),
    StorageLive(LocalId),
    StorageDead(LocalId),
    Deinit(Place),
    Nop,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Statement {
    pub kind: StatementKind,
    pub source_span: (usize, usize),
}

#[derive(Debug, Clone, PartialEq)]
pub enum TerminatorKind {
    Goto {
        target: BasicBlockId,
    },
    SwitchInt {
        discr: Operand,
        targets: Vec<(i64, BasicBlockId)>,
        otherwise: BasicBlockId,
    },
    Return,
    Unreachable,
    Drop {
        place: Place,
        target: BasicBlockId,
        unwind: Option<BasicBlockId>,
    },
    Call {
        callee: String,
        args: Vec<Operand>,
        destination: Place,
        target: BasicBlockId,
        cleanup: Option<BasicBlockId>,
    },
    Assert {
        cond: Operand,
        expected: bool,
        msg: String,
        target: BasicBlockId,
        cleanup: Option<BasicBlockId>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Terminator {
    pub kind: TerminatorKind,
    pub source_span: (usize, usize),
}

#[derive(Debug, Clone, Default)]
pub struct BasicBlockData {
    pub statements: Vec<Statement>,
    pub terminator: Option<Terminator>,
    pub is_cleanup: bool,
}

#[derive(Debug, Clone)]
pub struct LocalDecl {
    pub name: Option<String>,
    pub type_name: String,
    pub field_types: Vec<String>,
    pub is_mut: bool,
    pub is_user_variable: bool,
    pub global_symbol: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MirGlobal {
    pub name: String,
    pub type_name: String,
    pub mutable: bool,
    pub public: bool,
    pub value: MirGlobalValue,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MirGlobalValue {
    Integer(i64),
    Boolean(bool),
    String(String),
}

#[derive(Debug, Clone)]
pub struct MirBody {
    pub basic_blocks: Vec<BasicBlockData>,
    pub local_decls: Vec<LocalDecl>,
    pub globals: Vec<MirGlobal>,
    pub arg_count: usize,
}

impl MirBody {
    pub fn new(arg_count: usize) -> Self {
        Self {
            basic_blocks: Vec::new(),
            local_decls: Vec::new(),
            globals: Vec::new(),
            arg_count,
        }
    }

    pub fn return_place() -> Place {
        Place::from_local(LocalId(0))
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.local_decls.len() <= self.arg_count {
            return Err("return place and all parameter locals must be declared".to_owned());
        }
        if self.basic_blocks.is_empty() {
            return Err("MIR body has no entry block".to_owned());
        }

        for (block_index, block) in self.basic_blocks.iter().enumerate() {
            let Some(terminator) = &block.terminator else {
                return Err(format!("basic block {block_index} has no terminator"));
            };
            for statement in &block.statements {
                match &statement.kind {
                    StatementKind::Assign(place, value) => {
                        self.validate_place(place)?;
                        self.validate_rvalue(value)?;
                        if let Rvalue::Ref(kind, borrowed) = value {
                            self.validate_borrow_assignment(place, *kind, borrowed)?;
                        }
                    }
                    StatementKind::StorageLive(local) | StatementKind::StorageDead(local) => {
                        self.validate_local(*local)?;
                    }
                    StatementKind::Deinit(place) => self.validate_place(place)?,
                    StatementKind::Nop => {}
                }
            }
            match &terminator.kind {
                TerminatorKind::Goto { target } => self.validate_target(*target)?,
                TerminatorKind::SwitchInt {
                    discr,
                    targets,
                    otherwise,
                } => {
                    self.validate_operand(discr)?;
                    self.validate_target(*otherwise)?;
                    for (_, target) in targets {
                        self.validate_target(*target)?;
                    }
                }
                TerminatorKind::Return | TerminatorKind::Unreachable => {}
                TerminatorKind::Drop {
                    place,
                    target,
                    unwind,
                } => {
                    self.validate_place(place)?;
                    self.validate_target(*target)?;
                    if let Some(unwind) = unwind {
                        self.validate_target(*unwind)?;
                    }
                }
                TerminatorKind::Call {
                    args,
                    destination,
                    target,
                    cleanup,
                    ..
                } => {
                    for argument in args {
                        self.validate_operand(argument)?;
                    }
                    self.validate_place(destination)?;
                    self.validate_target(*target)?;
                    if let Some(cleanup) = cleanup {
                        self.validate_target(*cleanup)?;
                    }
                }
                TerminatorKind::Assert {
                    cond,
                    target,
                    cleanup,
                    ..
                } => {
                    self.validate_operand(cond)?;
                    self.validate_target(*target)?;
                    if let Some(cleanup) = cleanup {
                        self.validate_target(*cleanup)?;
                    }
                }
            }
        }
        self.validate_definite_assignment()?;
        self.validate_ownership()
    }

    fn validate_definite_assignment(&self) -> Result<(), String> {
        let reachable = self.reachable_blocks()?;
        let predecessors = self.block_predecessors();
        let initial = InitializationState {
            initialized: (1..=self.arg_count)
                .map(LocalId)
                .chain(
                    self.local_decls
                        .iter()
                        .enumerate()
                        .filter(|(_, declaration)| declaration.global_symbol.is_some())
                        .map(|(index, _)| LocalId(index)),
                )
                .collect(),
            moved: HashSet::new(),
        };
        let universe = InitializationState {
            initialized: (0..self.local_decls.len()).map(LocalId).collect(),
            moved: HashSet::new(),
        };
        let mut incoming = vec![universe.clone(); self.basic_blocks.len()];
        let mut outgoing = vec![universe; self.basic_blocks.len()];
        incoming[0] = initial.clone();

        loop {
            let mut changed = false;
            for block_index in reachable.iter().copied() {
                let mut relevant_predecessors = predecessors[block_index]
                    .iter()
                    .copied()
                    .filter(|predecessor| reachable.contains(predecessor));
                let mut next_in = if block_index == 0 {
                    initial.clone()
                } else {
                    relevant_predecessors.next().map_or_else(
                        InitializationState::default,
                        |predecessor| {
                            self.initialized_on_edge(
                                predecessor,
                                block_index,
                                &outgoing[predecessor],
                            )
                        },
                    )
                };
                for predecessor in relevant_predecessors {
                    let edge_state =
                        self.initialized_on_edge(predecessor, block_index, &outgoing[predecessor]);
                    next_in.intersect_with(&edge_state);
                }
                if block_index == 0 {
                    next_in.intersect_with(&initial);
                }
                if incoming[block_index] != next_in {
                    incoming[block_index] = next_in;
                    changed = true;
                }
            }

            for block_index in reachable.iter().copied() {
                let mut next_out = incoming[block_index].clone();
                for statement in &self.basic_blocks[block_index].statements {
                    apply_initialization_effect(&statement.kind, &mut next_out);
                }
                if outgoing[block_index] != next_out {
                    outgoing[block_index] = next_out;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }

        for block_index in reachable {
            let block = &self.basic_blocks[block_index];
            let mut initialized = incoming[block_index].clone();
            for (statement_index, statement) in block.statements.iter().enumerate() {
                validate_statement_reads(
                    &statement.kind,
                    self,
                    &initialized,
                    block_index,
                    statement_index,
                )?;
                apply_initialization_effect(&statement.kind, &mut initialized);
            }
            let terminator = block
                .terminator
                .as_ref()
                .expect("MIR validation checked terminators");
            validate_terminator_reads(
                &terminator.kind,
                self,
                &initialized,
                block_index,
                block.statements.len(),
            )?;
            if matches!(terminator.kind, TerminatorKind::Return)
                && self.local_decls[0].type_name != "void"
                && !initialized.contains(LocalId(0))
            {
                return Err(format!(
                    "return in basic block {block_index} has no definitely initialized return value"
                ));
            }
        }
        Ok(())
    }

    fn validate_ownership(&self) -> Result<(), String> {
        let reachable = self.reachable_blocks()?;
        let predecessors = self.block_predecessors();
        let liveness = self.local_liveness(&reachable);
        let mut incoming = vec![HashSet::new(); self.basic_blocks.len()];
        let mut outgoing = incoming.clone();
        let entry_loans = (1..=self.arg_count)
            .filter_map(|index| {
                let local = LocalId(index);
                let type_name = &self.local_decls[index].type_name;
                is_reference_type(type_name).then(|| ActiveLoan {
                    reference: local,
                    parent: None,
                    kind: if type_name.starts_with("&mut ") {
                        BorrowKind::Mut
                    } else {
                        BorrowKind::Shared
                    },
                    place: Place::from_local(local).project_deref(),
                })
            })
            .collect::<HashSet<_>>();

        loop {
            let mut changed = false;
            for block_index in reachable.iter().copied() {
                let mut next_in = if block_index == 0 {
                    entry_loans.clone()
                } else {
                    HashSet::new()
                };
                for predecessor in predecessors[block_index]
                    .iter()
                    .copied()
                    .filter(|predecessor| reachable.contains(predecessor))
                {
                    next_in.extend(outgoing[predecessor].iter().cloned());
                }
                retain_live_loans(&mut next_in, &liveness[block_index].live_in);
                let mut next_out = next_in.clone();
                for (statement_index, statement) in
                    self.basic_blocks[block_index].statements.iter().enumerate()
                {
                    apply_loan_effect(&statement.kind, self, &mut next_out);
                    retain_live_loans(
                        &mut next_out,
                        &liveness[block_index].live_after_statements[statement_index],
                    );
                }
                retain_live_loans(&mut next_out, &liveness[block_index].live_out);
                if incoming[block_index] != next_in {
                    incoming[block_index] = next_in;
                    changed = true;
                }
                if outgoing[block_index] != next_out {
                    outgoing[block_index] = next_out;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }

        for block_index in reachable {
            let block = &self.basic_blocks[block_index];
            let mut loans = incoming[block_index].clone();
            retain_live_loans(&mut loans, &liveness[block_index].live_in);
            for (statement_index, statement) in block.statements.iter().enumerate() {
                retain_live_loans(
                    &mut loans,
                    &liveness[block_index].live_before_statements[statement_index],
                );
                validate_loan_access(
                    &statement.kind,
                    self,
                    &mut loans,
                    block_index,
                    statement_index,
                )?;
                apply_loan_effect(&statement.kind, self, &mut loans);
                retain_live_loans(
                    &mut loans,
                    &liveness[block_index].live_after_statements[statement_index],
                );
            }
            let terminator = &block
                .terminator
                .as_ref()
                .expect("MIR validation checked terminators")
                .kind;
            retain_live_loans(&mut loans, &liveness[block_index].live_before_terminator);
            validate_terminator_loan_access(terminator, self, &loans, block_index)?;
        }
        Ok(())
    }

    fn local_liveness(&self, reachable: &HashSet<usize>) -> Vec<BlockLiveness> {
        let mut live_in = vec![HashSet::new(); self.basic_blocks.len()];
        let mut live_out = live_in.clone();

        loop {
            let mut changed = false;
            for block_index in reachable.iter().copied() {
                let block = &self.basic_blocks[block_index];
                let mut next_out = HashSet::new();
                if let Some(terminator) = &block.terminator {
                    for target in terminator_targets(&terminator.kind) {
                        if reachable.contains(&target.0) {
                            next_out.extend(live_in[target.0].iter().copied());
                        }
                    }
                }

                let mut live = next_out.clone();
                if let Some(terminator) = &block.terminator {
                    let (uses, definitions) =
                        terminator_local_uses_and_definitions(&terminator.kind, self);
                    live.retain(|local| !definitions.contains(local));
                    live.extend(uses);
                }
                for statement in block.statements.iter().rev() {
                    let (uses, definitions) = statement_local_uses_and_definitions(&statement.kind);
                    live.retain(|local| !definitions.contains(local));
                    live.extend(uses);
                }
                if live_in[block_index] != live {
                    live_in[block_index] = live;
                    changed = true;
                }
                if live_out[block_index] != next_out {
                    live_out[block_index] = next_out;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }

        self.basic_blocks
            .iter()
            .enumerate()
            .map(|(block_index, block)| {
                let terminator = block.terminator.as_ref().map(|terminator| {
                    terminator_local_uses_and_definitions(&terminator.kind, self)
                });
                let mut live = live_out[block_index].clone();
                if let Some((uses, definitions)) = terminator {
                    live.retain(|local| !definitions.contains(local));
                    live.extend(uses);
                }
                let live_before_terminator = live.clone();
                let mut live_before_statements = vec![HashSet::new(); block.statements.len()];
                let mut live_after_statements = vec![HashSet::new(); block.statements.len()];
                for statement_index in (0..block.statements.len()).rev() {
                    live_after_statements[statement_index] = live.clone();
                    live_before_statements[statement_index] = live.clone();
                    let (uses, definitions) = statement_local_uses_and_definitions(
                        &block.statements[statement_index].kind,
                    );
                    live_before_statements[statement_index]
                        .retain(|local| !definitions.contains(local));
                    live_before_statements[statement_index].extend(uses);
                    live = live_before_statements[statement_index].clone();
                }
                BlockLiveness {
                    live_in: live_in[block_index].clone(),
                    live_out: live_out[block_index].clone(),
                    live_before_terminator,
                    live_before_statements,
                    live_after_statements,
                }
            })
            .collect()
    }

    fn initialized_on_edge(
        &self,
        predecessor: usize,
        target: usize,
        initialized: &InitializationState,
    ) -> InitializationState {
        let terminator = self.basic_blocks[predecessor]
            .terminator
            .as_ref()
            .expect("MIR validation checked terminators");
        let mut edge_state = initialized.clone();
        apply_terminator_consumption(&terminator.kind, &mut edge_state);
        if let TerminatorKind::Call {
            destination,
            target: normal_target,
            ..
        } = &terminator.kind
            && normal_target.0 == target
            && destination.projection.is_empty()
        {
            edge_state.assign(destination);
        }
        edge_state
    }

    fn reachable_blocks(&self) -> Result<HashSet<usize>, String> {
        let mut reachable = HashSet::new();
        let mut pending = VecDeque::from([0usize]);
        while let Some(block_index) = pending.pop_front() {
            if !reachable.insert(block_index) {
                continue;
            }
            let block = &self.basic_blocks[block_index];
            let terminator = block
                .terminator
                .as_ref()
                .expect("MIR validation checked terminators");
            match &terminator.kind {
                TerminatorKind::SwitchInt {
                    discr: Operand::ConstantInt(value),
                    targets,
                    otherwise,
                } => pending.push_back(
                    targets
                        .iter()
                        .find(|(candidate, _)| candidate == value)
                        .map_or(*otherwise, |(_, target)| *target)
                        .0,
                ),
                _ => pending.extend(terminator_targets(&terminator.kind).map(|target| target.0)),
            }
        }
        Ok(reachable)
    }

    fn block_predecessors(&self) -> Vec<Vec<usize>> {
        let mut predecessors = vec![Vec::new(); self.basic_blocks.len()];
        for (block_index, block) in self.basic_blocks.iter().enumerate() {
            if let Some(terminator) = &block.terminator {
                for target in terminator_targets(&terminator.kind) {
                    predecessors[target.0].push(block_index);
                }
            }
        }
        predecessors
    }

    fn validate_local(&self, local: LocalId) -> Result<(), String> {
        if local.0 < self.local_decls.len() {
            Ok(())
        } else {
            Err(format!("local _L{} is not declared", local.0))
        }
    }

    fn validate_place(&self, place: &Place) -> Result<(), String> {
        self.validate_local(place.local)?;
        for (index, projection) in place.projection.iter().enumerate() {
            if let ProjectionElem::Index(local) = projection {
                self.validate_local(*local)?;
            }
            if let ProjectionElem::Field(field) = projection
                && index == 0
                && field.0 >= self.local_decls[place.local.0].field_types.len()
            {
                return Err(format!(
                    "field {} is not declared for local _L{}",
                    field.0, place.local.0
                ));
            }
        }
        Ok(())
    }

    fn validate_operand(&self, operand: &Operand) -> Result<(), String> {
        match operand {
            Operand::Copy(place) => {
                self.validate_place(place)?;
                if place.projection.is_empty()
                    && self.local_decls[place.local.0]
                        .type_name
                        .starts_with("&mut ")
                {
                    return Err(format!(
                        "mutable reference local _L{} cannot be copied",
                        place.local.0
                    ));
                }
                Ok(())
            }
            Operand::Move(place) => self.validate_place(place),
            Operand::ConstantInt(_)
            | Operand::ConstantFloat(_)
            | Operand::ConstantBool(_)
            | Operand::ConstantString(_) => Ok(()),
        }
    }

    fn validate_rvalue(&self, value: &Rvalue) -> Result<(), String> {
        match value {
            Rvalue::Use(operand) | Rvalue::Cast(operand, _) => self.validate_operand(operand),
            Rvalue::Ref(_, place) | Rvalue::Discriminant(place) => self.validate_place(place),
            Rvalue::BinaryOp(_, left, right) => {
                self.validate_operand(left)?;
                self.validate_operand(right)
            }
            Rvalue::Aggregate(operands) => {
                for operand in operands {
                    self.validate_operand(operand)?;
                }
                Ok(())
            }
        }
    }

    fn validate_borrow_assignment(
        &self,
        destination: &Place,
        kind: BorrowKind,
        borrowed: &Place,
    ) -> Result<(), String> {
        let destination_type = &self.local_decls[destination.local.0].type_name;
        let referent_type = if borrowed.projection.is_empty() {
            Some(self.local_decls[borrowed.local.0].type_name.as_str())
        } else if borrowed.projection == [ProjectionElem::Deref] {
            self.local_decls[borrowed.local.0]
                .type_name
                .strip_prefix("&mut ")
                .or_else(|| {
                    self.local_decls[borrowed.local.0]
                        .type_name
                        .strip_prefix('&')
                })
        } else if let [ProjectionElem::Field(field)] = borrowed.projection.as_slice() {
            self.local_decls[borrowed.local.0]
                .field_types
                .get(field.0)
                .map(String::as_str)
        } else {
            None
        };
        let prefix = if kind == BorrowKind::Mut {
            "&mut "
        } else {
            "&"
        };
        if !destination.projection.is_empty()
            || destination_type.strip_prefix(prefix) != referent_type
        {
            return Err(format!(
                "reference assignment to _L{} does not match the borrowed local _L{} type",
                destination.local.0, borrowed.local.0
            ));
        }
        Ok(())
    }

    fn validate_target(&self, target: BasicBlockId) -> Result<(), String> {
        if target.0 < self.basic_blocks.len() {
            Ok(())
        } else {
            Err(format!("basic block target BB{} is not declared", target.0))
        }
    }
}

fn terminator_targets(kind: &TerminatorKind) -> impl Iterator<Item = BasicBlockId> + '_ {
    let mut targets = Vec::new();
    match kind {
        TerminatorKind::Goto { target } => targets.push(*target),
        TerminatorKind::SwitchInt {
            discr: Operand::ConstantInt(value),
            targets: cases,
            otherwise,
        } => targets.push(
            cases
                .iter()
                .find(|(candidate, _)| candidate == value)
                .map_or(*otherwise, |(_, target)| *target),
        ),
        TerminatorKind::SwitchInt {
            targets: cases,
            otherwise,
            ..
        } => {
            targets.extend(cases.iter().map(|(_, target)| *target));
            targets.push(*otherwise);
        }
        TerminatorKind::Drop { target, unwind, .. } => {
            targets.push(*target);
            targets.extend(unwind.iter().copied());
        }
        TerminatorKind::Call {
            target, cleanup, ..
        }
        | TerminatorKind::Assert {
            target, cleanup, ..
        } => {
            targets.push(*target);
            targets.extend(cleanup.iter().copied());
        }
        TerminatorKind::Return | TerminatorKind::Unreachable => {}
    }
    targets.into_iter()
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct InitializationState {
    initialized: HashSet<LocalId>,
    moved: HashSet<Place>,
}

impl InitializationState {
    fn contains(&self, local: LocalId) -> bool {
        self.initialized.contains(&local)
    }

    fn assign(&mut self, place: &Place) {
        if place.projection.is_empty() {
            self.initialized.insert(place.local);
            self.moved.retain(|moved| moved.local != place.local);
        } else {
            self.moved.retain(|moved| !places_overlap(moved, place));
        }
    }

    fn deinitialize(&mut self, place: &Place) {
        if place.projection.is_empty() {
            self.initialized.remove(&place.local);
            self.moved.retain(|moved| moved.local != place.local);
        } else {
            self.moved.insert(place.clone());
        }
    }

    fn move_out(&mut self, place: &Place) {
        self.deinitialize(place);
    }

    fn is_initialized(&self, place: &Place) -> bool {
        self.initialized.contains(&place.local)
            && !self.moved.iter().any(|moved| places_overlap(moved, place))
    }

    fn intersect_with(&mut self, other: &Self) {
        self.initialized
            .retain(|local| other.initialized.contains(local));
        self.moved.extend(other.moved.iter().cloned());
    }
}

fn apply_initialization_effect(kind: &StatementKind, initialized: &mut InitializationState) {
    match kind {
        StatementKind::Assign(place, value) => {
            apply_rvalue_moves(value, initialized);
            initialized.assign(place);
        }
        StatementKind::Nop => {}
        StatementKind::StorageLive(local) | StatementKind::StorageDead(local) => {
            initialized.deinitialize(&Place::from_local(*local));
        }
        StatementKind::Deinit(place) => {
            initialized.deinitialize(place);
        }
    }
}

fn apply_terminator_consumption(kind: &TerminatorKind, initialized: &mut InitializationState) {
    match kind {
        TerminatorKind::SwitchInt { discr, .. } | TerminatorKind::Assert { cond: discr, .. } => {
            apply_operand_move(discr, initialized);
        }
        TerminatorKind::Drop { place, .. } => {
            initialized.deinitialize(place);
        }
        TerminatorKind::Call { args, .. } => {
            for argument in args {
                apply_operand_move(argument, initialized);
            }
        }
        TerminatorKind::Goto { .. } | TerminatorKind::Return | TerminatorKind::Unreachable => {}
    }
}

fn apply_rvalue_moves(value: &Rvalue, initialized: &mut InitializationState) {
    match value {
        Rvalue::Use(operand) | Rvalue::Cast(operand, _) => {
            apply_operand_move(operand, initialized);
        }
        Rvalue::BinaryOp(_, left, right) => {
            apply_operand_move(left, initialized);
            apply_operand_move(right, initialized);
        }
        Rvalue::Aggregate(operands) => {
            for operand in operands {
                apply_operand_move(operand, initialized);
            }
        }
        Rvalue::Ref(_, _) | Rvalue::Discriminant(_) => {}
    }
}

fn apply_operand_move(operand: &Operand, initialized: &mut InitializationState) {
    if let Operand::Move(place) = operand {
        initialized.move_out(place);
    }
}

fn validate_statement_reads(
    kind: &StatementKind,
    body: &MirBody,
    initialized: &InitializationState,
    block: usize,
    statement: usize,
) -> Result<(), String> {
    let point = format!("basic block {block}, statement {statement}");
    match kind {
        StatementKind::Assign(destination, value) => {
            if !destination.projection.is_empty() {
                require_local_initialized(body, destination.local, initialized, &point)?;
            }
            validate_rvalue_reads(value, body, initialized, &point)
        }
        StatementKind::Deinit(place) => require_initialized(body, place, initialized, &point),
        StatementKind::StorageLive(_) | StatementKind::StorageDead(_) | StatementKind::Nop => {
            Ok(())
        }
    }
}

fn validate_terminator_reads(
    kind: &TerminatorKind,
    body: &MirBody,
    initialized: &InitializationState,
    block: usize,
    statement: usize,
) -> Result<(), String> {
    let point = format!("basic block {block}, terminator after statement {statement}");
    match kind {
        TerminatorKind::SwitchInt { discr, .. } => {
            validate_operand_reads(discr, body, initialized, &point)
        }
        TerminatorKind::Drop { place, .. } => require_initialized(body, place, initialized, &point),
        TerminatorKind::Call {
            args, destination, ..
        } => {
            for argument in args {
                validate_operand_reads(argument, body, initialized, &point)?;
            }
            if !destination.projection.is_empty() {
                require_local_initialized(body, destination.local, initialized, &point)?;
            }
            Ok(())
        }
        TerminatorKind::Assert { cond, .. } => {
            validate_operand_reads(cond, body, initialized, &point)
        }
        TerminatorKind::Return | TerminatorKind::Goto { .. } | TerminatorKind::Unreachable => {
            Ok(())
        }
    }
}

fn validate_rvalue_reads(
    value: &Rvalue,
    body: &MirBody,
    initialized: &InitializationState,
    point: &str,
) -> Result<(), String> {
    match value {
        Rvalue::Use(operand) | Rvalue::Cast(operand, _) => {
            validate_operand_reads(operand, body, initialized, point)
        }
        Rvalue::Ref(_, place) | Rvalue::Discriminant(place) => {
            require_initialized(body, place, initialized, point)
        }
        Rvalue::BinaryOp(_, left, right) => {
            validate_operand_reads(left, body, initialized, point)?;
            validate_operand_reads(right, body, initialized, point)
        }
        Rvalue::Aggregate(operands) => {
            for operand in operands {
                validate_operand_reads(operand, body, initialized, point)?;
            }
            Ok(())
        }
    }
}

fn validate_operand_reads(
    operand: &Operand,
    body: &MirBody,
    initialized: &InitializationState,
    point: &str,
) -> Result<(), String> {
    match operand {
        Operand::Copy(place) | Operand::Move(place) => {
            require_initialized(body, place, initialized, point)
        }
        Operand::ConstantInt(_)
        | Operand::ConstantFloat(_)
        | Operand::ConstantBool(_)
        | Operand::ConstantString(_) => Ok(()),
    }
}

fn require_initialized(
    body: &MirBody,
    place: &Place,
    initialized: &InitializationState,
    point: &str,
) -> Result<(), String> {
    if initialized.is_initialized(place) {
        Ok(())
    } else {
        Err(format!(
            "place _L{} ({}) may be uninitialized or partially moved at {point}",
            place.local.0,
            body.local_decls[place.local.0]
                .name
                .as_deref()
                .unwrap_or("unnamed"),
        ))
    }
}

fn require_local_initialized(
    body: &MirBody,
    local: LocalId,
    initialized: &InitializationState,
    point: &str,
) -> Result<(), String> {
    if initialized.contains(local) {
        Ok(())
    } else {
        Err(format!(
            "local _L{} ({}) may be uninitialized at {point}",
            local.0,
            body.local_decls[local.0]
                .name
                .as_deref()
                .unwrap_or("unnamed"),
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ActiveLoan {
    reference: LocalId,
    parent: Option<LocalId>,
    kind: BorrowKind,
    place: Place,
}

#[derive(Default)]
struct BlockLiveness {
    live_in: HashSet<LocalId>,
    live_out: HashSet<LocalId>,
    live_before_terminator: HashSet<LocalId>,
    live_before_statements: Vec<HashSet<LocalId>>,
    live_after_statements: Vec<HashSet<LocalId>>,
}

fn retain_live_loans(loans: &mut HashSet<ActiveLoan>, live: &HashSet<LocalId>) {
    loans.retain(|loan| live.contains(&loan.reference));
}

fn add_place_locals(place: &Place, locals: &mut HashSet<LocalId>) {
    locals.insert(place.local);
    for projection in &place.projection {
        if let ProjectionElem::Index(local) = projection {
            locals.insert(*local);
        }
    }
}

fn add_operand_locals(operand: &Operand, locals: &mut HashSet<LocalId>) {
    if let Operand::Copy(place) | Operand::Move(place) = operand {
        add_place_locals(place, locals);
    }
}

fn add_rvalue_locals(value: &Rvalue, locals: &mut HashSet<LocalId>) {
    match value {
        Rvalue::Use(operand) | Rvalue::Cast(operand, _) => add_operand_locals(operand, locals),
        Rvalue::Ref(_, place) | Rvalue::Discriminant(place) => add_place_locals(place, locals),
        Rvalue::BinaryOp(_, left, right) => {
            add_operand_locals(left, locals);
            add_operand_locals(right, locals);
        }
        Rvalue::Aggregate(operands) => {
            for operand in operands {
                add_operand_locals(operand, locals);
            }
        }
    }
}

fn statement_local_uses_and_definitions(
    statement: &StatementKind,
) -> (HashSet<LocalId>, HashSet<LocalId>) {
    let mut uses = HashSet::new();
    let mut definitions = HashSet::new();
    match statement {
        StatementKind::Assign(destination, value) => {
            add_rvalue_locals(value, &mut uses);
            if destination.projection.is_empty() {
                definitions.insert(destination.local);
            } else {
                add_place_locals(destination, &mut uses);
            }
        }
        StatementKind::StorageLive(local) | StatementKind::StorageDead(local) => {
            definitions.insert(*local);
        }
        StatementKind::Deinit(place) => add_place_locals(place, &mut uses),
        StatementKind::Nop => {}
    }
    (uses, definitions)
}

fn terminator_local_uses_and_definitions(
    terminator: &TerminatorKind,
    body: &MirBody,
) -> (HashSet<LocalId>, HashSet<LocalId>) {
    let mut uses = HashSet::new();
    let mut definitions = HashSet::new();
    match terminator {
        TerminatorKind::SwitchInt { discr, .. } => add_operand_locals(discr, &mut uses),
        TerminatorKind::Return if body.local_decls[0].type_name != "void" => {
            uses.insert(LocalId(0));
        }
        TerminatorKind::Drop { place, .. } => add_place_locals(place, &mut uses),
        TerminatorKind::Call {
            args, destination, ..
        } => {
            for argument in args {
                add_operand_locals(argument, &mut uses);
            }
            if destination.projection.is_empty() {
                definitions.insert(destination.local);
            } else {
                add_place_locals(destination, &mut uses);
            }
        }
        TerminatorKind::Assert { cond, .. } => add_operand_locals(cond, &mut uses),
        TerminatorKind::Goto { .. } | TerminatorKind::Return | TerminatorKind::Unreachable => {}
    }
    (uses, definitions)
}

fn is_reference_type(type_name: &str) -> bool {
    type_name.starts_with('&')
}

fn places_overlap(left: &Place, right: &Place) -> bool {
    if left.local != right.local {
        return false;
    }
    for (left_projection, right_projection) in left.projection.iter().zip(&right.projection) {
        match (left_projection, right_projection) {
            (ProjectionElem::Field(left), ProjectionElem::Field(right)) if left != right => {
                return false;
            }
            (
                ProjectionElem::Subslice {
                    from: left_from,
                    to: left_to,
                },
                ProjectionElem::Subslice {
                    from: right_from,
                    to: right_to,
                },
            ) if left_to <= right_from || right_to <= left_from => return false,
            _ => {}
        }
    }
    true
}

fn apply_loan_effect(kind: &StatementKind, body: &MirBody, loans: &mut HashSet<ActiveLoan>) {
    match kind {
        StatementKind::StorageLive(local) | StatementKind::StorageDead(local) => {
            loans.retain(|loan| loan.reference != *local);
        }
        StatementKind::Assign(destination, value) if destination.projection.is_empty() => {
            loans.retain(|loan| loan.reference != destination.local);
            if let Rvalue::Ref(kind, place) = value
                && body
                    .local_decls
                    .get(destination.local.0)
                    .is_some_and(|decl| is_reference_type(&decl.type_name))
            {
                let (place, parent) = resolved_place(place, loans);
                loans.insert(ActiveLoan {
                    reference: destination.local,
                    parent,
                    kind: *kind,
                    place,
                });
            } else if let Rvalue::Use(Operand::Copy(source)) = value {
                loans.extend(inherited_loans(loans, destination.local, source.local));
            } else if let Rvalue::Use(Operand::Move(source)) = value {
                loans.extend(inherited_loans(loans, destination.local, source.local));
                loans.retain(|loan| loan.reference != source.local);
            }
        }
        StatementKind::Deinit(place) if place.projection.is_empty() => {
            loans.retain(|loan| loan.reference != place.local);
        }
        _ => {}
    }
}

fn inherited_loans(
    loans: &HashSet<ActiveLoan>,
    destination: LocalId,
    source: LocalId,
) -> Vec<ActiveLoan> {
    loans
        .iter()
        .filter(|loan| loan.reference == source)
        .cloned()
        .map(|mut loan| {
            loan.reference = destination;
            loan
        })
        .collect()
}

fn validate_loan_access(
    kind: &StatementKind,
    body: &MirBody,
    loans: &mut HashSet<ActiveLoan>,
    block: usize,
    statement: usize,
) -> Result<(), String> {
    match kind {
        StatementKind::Assign(destination, value) => {
            if destination.projection.is_empty() {
                loans.retain(|loan| loan.reference != destination.local);
            }
            check_loan_access(body, destination, true, false, loans, block, statement)?;
            match value {
                Rvalue::Ref(kind, place) => {
                    let (resolved, parent) = resolved_place(place, loans);
                    let parent_loan = parent.and_then(|reference| {
                        loans.iter().find(|loan| loan.reference == reference)
                    });
                    if *kind == BorrowKind::Mut
                        && match parent_loan {
                            Some(loan) => loan.kind != BorrowKind::Mut,
                            None => !body.local_decls[place.local.0].is_mut,
                        }
                    {
                        return Err(format!(
                            "mutable borrow of immutable or shared-referenced local _L{} at basic block {block}, statement {statement}",
                            place.local.0
                        ));
                    }
                    for loan in loans.iter() {
                        if Some(loan.reference) != parent
                            && !parent
                                .is_some_and(|parent| is_ancestor(loan.reference, parent, loans))
                            && places_overlap(&loan.place, &resolved)
                            && (*kind == BorrowKind::Mut || loan.kind == BorrowKind::Mut)
                        {
                            return Err(format!(
                                "conflicting {:?} borrow of local _L{} overlaps active {:?} borrow held by _L{} at basic block {block}, statement {statement}",
                                kind, place.local.0, loan.kind, loan.reference.0
                            ));
                        }
                    }
                }
                _ => validate_rvalue_loan_access(value, body, loans, block, statement)?,
            }
        }
        StatementKind::Deinit(place) => {
            check_loan_access(body, place, true, true, loans, block, statement)?;
        }
        StatementKind::StorageDead(local) => {
            if let Some(loan) = loans.iter().find(|loan| loan.place.local == *local) {
                return Err(format!(
                    "local _L{} leaves its storage region while borrowed by _L{} at basic block {block}, statement {statement}",
                    local.0, loan.reference.0
                ));
            }
        }
        StatementKind::StorageLive(_) | StatementKind::Nop => {}
    }
    Ok(())
}

fn validate_rvalue_loan_access(
    value: &Rvalue,
    body: &MirBody,
    loans: &HashSet<ActiveLoan>,
    block: usize,
    statement: usize,
) -> Result<(), String> {
    match value {
        Rvalue::Use(operand) | Rvalue::Cast(operand, _) => {
            validate_operand_loan_access(operand, body, loans, block, statement)
        }
        Rvalue::Ref(_, place) | Rvalue::Discriminant(place) => {
            check_loan_access(body, place, false, false, loans, block, statement)
        }
        Rvalue::BinaryOp(_, left, right) => {
            validate_operand_loan_access(left, body, loans, block, statement)?;
            validate_operand_loan_access(right, body, loans, block, statement)
        }
        Rvalue::Aggregate(operands) => {
            for operand in operands {
                validate_operand_loan_access(operand, body, loans, block, statement)?;
            }
            Ok(())
        }
    }
}

fn validate_operand_loan_access(
    operand: &Operand,
    body: &MirBody,
    loans: &HashSet<ActiveLoan>,
    block: usize,
    statement: usize,
) -> Result<(), String> {
    if let Operand::Copy(place) | Operand::Move(place) = operand {
        check_loan_access(
            body,
            place,
            false,
            matches!(operand, Operand::Move(_)),
            loans,
            block,
            statement,
        )?;
    }
    Ok(())
}

fn check_loan_access(
    body: &MirBody,
    place: &Place,
    is_write: bool,
    is_move: bool,
    loans: &HashSet<ActiveLoan>,
    block: usize,
    statement: usize,
) -> Result<(), String> {
    let (resolved, through_reference) = resolved_place(place, loans);
    if let Some(reference) = through_reference
        && (is_write || is_move)
        && loans
            .iter()
            .any(|loan| loan.reference == reference && loan.kind != BorrowKind::Mut)
    {
        return Err(format!(
            "{} through shared reference _L{} is not allowed at basic block {block}, statement {statement}",
            if is_move { "move" } else { "write" },
            reference.0
        ));
    }
    if let Some(reference) = through_reference
        && loans.iter().any(|loan| {
            loan.reference != reference
                && is_ancestor(reference, loan.reference, loans)
                && places_overlap(&loan.place, &resolved)
        })
    {
        return Err(format!(
            "access through _L{} conflicts with an active reborrow at basic block {block}, statement {statement}",
            reference.0
        ));
    }
    for loan in loans.iter().filter(|loan| {
        Some(loan.reference) != through_reference
            && !through_reference
                .is_some_and(|reference| is_ancestor(loan.reference, reference, loans))
            && places_overlap(&loan.place, &resolved)
    }) {
        if loan.kind == BorrowKind::Mut || is_write || is_move {
            let name = body.local_decls[place.local.0]
                .name
                .as_deref()
                .unwrap_or("unnamed");
            return Err(format!(
                "{} of local _L{} ({name}) conflicts with active {:?} borrow held by _L{} at basic block {block}, statement {statement}",
                if is_move {
                    "move"
                } else if is_write {
                    "write"
                } else {
                    "read"
                },
                place.local.0,
                loan.kind,
                loan.reference.0
            ));
        }
    }
    Ok(())
}

fn validate_terminator_loan_access(
    kind: &TerminatorKind,
    body: &MirBody,
    loans: &HashSet<ActiveLoan>,
    block: usize,
) -> Result<(), String> {
    match kind {
        TerminatorKind::Drop { place, .. } => {
            check_loan_access(body, place, true, true, loans, block, usize::MAX)
        }
        TerminatorKind::Call { args, .. } => {
            for argument in args {
                validate_operand_loan_access(argument, body, loans, block, usize::MAX)?;
            }
            validate_call_loans(args, body, loans, block)
        }
        TerminatorKind::SwitchInt { discr, .. } => {
            validate_operand_loan_access(discr, body, loans, block, usize::MAX)
        }
        TerminatorKind::Assert { cond, .. } => {
            validate_operand_loan_access(cond, body, loans, block, usize::MAX)
        }
        TerminatorKind::Return if is_reference_type(&body.local_decls[0].type_name) => {
            let return_loans = loans
                .iter()
                .filter(|loan| loan.reference == LocalId(0))
                .collect::<Vec<_>>();
            if return_loans.is_empty()
                || return_loans.iter().any(|loan| {
                    loan.place.local.0 == 0
                        || loan.place.local.0 > body.arg_count
                        || !matches!(loan.place.projection.first(), Some(ProjectionElem::Deref))
                        || !is_reference_type(&body.local_decls[loan.place.local.0].type_name)
                })
            {
                Err(format!(
                    "reference return in basic block {block} is not derived from a reference parameter"
                ))
            } else {
                Ok(())
            }
        }
        TerminatorKind::Goto { .. } | TerminatorKind::Return | TerminatorKind::Unreachable => {
            Ok(())
        }
    }
}

fn resolved_place(place: &Place, loans: &HashSet<ActiveLoan>) -> (Place, Option<LocalId>) {
    let Some(ProjectionElem::Deref) = place.projection.first() else {
        return (place.clone(), None);
    };
    let Some(loan) = loans.iter().find(|loan| loan.reference == place.local) else {
        return (place.clone(), Some(place.local));
    };
    let mut resolved = loan.place.clone();
    resolved
        .projection
        .extend(place.projection.iter().skip(1).cloned());
    (resolved, Some(place.local))
}

fn validate_call_loans(
    args: &[Operand],
    body: &MirBody,
    loans: &HashSet<ActiveLoan>,
    block: usize,
) -> Result<(), String> {
    let arguments = args
        .iter()
        .filter_map(|argument| {
            let place = match argument {
                Operand::Copy(place) | Operand::Move(place) if place.projection.is_empty() => place,
                _ => return None,
            };
            let type_name = &body.local_decls[place.local.0].type_name;
            if !is_reference_type(type_name) {
                return None;
            }
            let kind = if type_name.starts_with("&mut ") {
                BorrowKind::Mut
            } else {
                BorrowKind::Shared
            };
            let existing = loans.iter().find(|loan| loan.reference == place.local);
            let target = existing.map_or_else(
                || Place::from_local(place.local).project_deref(),
                |loan| loan.place.clone(),
            );
            Some((place.local, kind, target))
        })
        .collect::<Vec<_>>();

    for (index, (left_reference, left_kind, left_place)) in arguments.iter().enumerate() {
        for (right_reference, right_kind, right_place) in arguments.iter().skip(index + 1) {
            if places_overlap(left_place, right_place)
                && (*left_kind == BorrowKind::Mut || *right_kind == BorrowKind::Mut)
            {
                return Err(format!(
                    "call in basic block {block} passes overlapping references _L{} and _L{}",
                    left_reference.0, right_reference.0
                ));
            }
        }
        if loans.iter().any(|loan| {
            loan.reference != *left_reference
                && !is_ancestor(loan.reference, *left_reference, loans)
                && places_overlap(left_place, &loan.place)
                && (*left_kind == BorrowKind::Mut || loan.kind == BorrowKind::Mut)
        }) {
            return Err(format!(
                "call in basic block {block} uses reference _L{} while an overlapping {:?} loan is active",
                left_reference.0, left_kind
            ));
        }
    }

    if let TerminatorKind::Call { destination, .. } = &body.basic_blocks[block]
        .terminator
        .as_ref()
        .expect("MIR validation checked terminators")
        .kind
        && is_reference_type(&body.local_decls[destination.local.0].type_name)
    {
        return Err(format!(
            "reference result from call in basic block {block} has no return-lifetime metadata"
        ));
    }
    Ok(())
}

#[derive(Debug, Default)]
pub struct BorrowCheckerState;

impl BorrowCheckerState {
    pub fn check_body(&mut self, body: &MirBody) -> Vec<String> {
        body.validate_ownership().err().into_iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn local(name: &str, type_name: &str) -> LocalDecl {
        LocalDecl {
            name: Some(name.to_owned()),
            type_name: type_name.to_owned(),
            field_types: Vec::new(),
            is_mut: true,
            is_user_variable: true,
            global_symbol: None,
        }
    }

    fn terminator(kind: TerminatorKind) -> Terminator {
        Terminator {
            kind,
            source_span: (0, 0),
        }
    }

    fn loop_with_mutable_loan(use_after_loop: bool) -> MirBody {
        let mut body = MirBody::new(1);
        body.local_decls = vec![
            local("_return", "void"),
            local("condition", "bool"),
            local("value", "int"),
            local("reference", "&mut int"),
            local("read", "int"),
        ];
        let reference_read = Statement {
            kind: StatementKind::Assign(
                Place::from_local(LocalId(4)),
                Rvalue::Use(Operand::Copy(Place::from_local(LocalId(3)).project_deref())),
            ),
            source_span: (0, 0),
        };
        body.basic_blocks = vec![
            BasicBlockData {
                statements: vec![
                    Statement {
                        kind: StatementKind::Assign(
                            Place::from_local(LocalId(2)),
                            Rvalue::Use(Operand::ConstantInt(1)),
                        ),
                        source_span: (0, 0),
                    },
                    Statement {
                        kind: StatementKind::Assign(
                            Place::from_local(LocalId(3)),
                            Rvalue::Ref(BorrowKind::Mut, Place::from_local(LocalId(2))),
                        ),
                        source_span: (0, 0),
                    },
                    reference_read.clone(),
                ],
                terminator: Some(terminator(TerminatorKind::Goto {
                    target: BasicBlockId(1),
                })),
                is_cleanup: false,
            },
            BasicBlockData {
                statements: Vec::new(),
                terminator: Some(terminator(TerminatorKind::SwitchInt {
                    discr: Operand::Copy(Place::from_local(LocalId(1))),
                    targets: vec![(1, BasicBlockId(2))],
                    otherwise: BasicBlockId(3),
                })),
                is_cleanup: false,
            },
            BasicBlockData {
                statements: vec![Statement {
                    kind: StatementKind::Assign(
                        Place::from_local(LocalId(2)),
                        Rvalue::Use(Operand::ConstantInt(2)),
                    ),
                    source_span: (0, 0),
                }],
                terminator: Some(terminator(TerminatorKind::Goto {
                    target: BasicBlockId(1),
                })),
                is_cleanup: false,
            },
            BasicBlockData {
                statements: if use_after_loop {
                    vec![reference_read]
                } else {
                    Vec::new()
                },
                terminator: Some(terminator(TerminatorKind::Return)),
                is_cleanup: false,
            },
        ];
        body
    }

    #[test]
    fn mutable_and_shared_reference_loans_cannot_alias() {
        let mut body = MirBody::new(0);
        body.local_decls = vec![
            local("_return", "void"),
            local("value", "int"),
            local("shared", "&int"),
            local("exclusive", "&mut int"),
            local("observed", "int"),
        ];
        body.basic_blocks = vec![BasicBlockData {
            statements: vec![
                Statement {
                    kind: StatementKind::Assign(
                        Place::from_local(LocalId(1)),
                        Rvalue::Use(Operand::ConstantInt(1)),
                    ),
                    source_span: (0, 0),
                },
                Statement {
                    kind: StatementKind::Assign(
                        Place::from_local(LocalId(2)),
                        Rvalue::Ref(BorrowKind::Shared, Place::from_local(LocalId(1))),
                    ),
                    source_span: (0, 0),
                },
                Statement {
                    kind: StatementKind::Assign(
                        Place::from_local(LocalId(3)),
                        Rvalue::Ref(BorrowKind::Mut, Place::from_local(LocalId(1))),
                    ),
                    source_span: (0, 0),
                },
                Statement {
                    kind: StatementKind::Assign(
                        Place::from_local(LocalId(4)),
                        Rvalue::Use(Operand::Copy(Place::from_local(LocalId(2)).project_deref())),
                    ),
                    source_span: (0, 0),
                },
            ],
            terminator: Some(terminator(TerminatorKind::Return)),
            ..BasicBlockData::default()
        }];

        let mut checker = BorrowCheckerState;
        let errors = checker.check_body(&body);
        assert!(
            errors
                .iter()
                .any(|error| error.contains("conflicting Mut borrow"))
        );
    }

    #[test]
    fn loan_expires_before_loop_when_reference_has_no_later_uses() {
        assert!(
            loop_with_mutable_loan(false).validate().is_ok(),
            "a loan whose final use precedes the loop should not block loop writes"
        );
    }

    #[test]
    fn loan_stays_live_across_loop_backedge_when_used_after_loop() {
        let result = loop_with_mutable_loan(true).validate();
        assert!(
            matches!(&result, Err(error) if error.contains("conflicts with active")),
            "a reference used after the loop must keep its loan live across the backedge: {result:?}"
        );
    }

    #[test]
    fn mutable_reborrows_suspend_access_through_the_parent() {
        let mut body = MirBody::new(0);
        body.local_decls = vec![
            local("_return", "void"),
            local("value", "int"),
            local("parent", "&mut int"),
            local("child", "&mut int"),
            local("observed", "int"),
        ];
        body.basic_blocks = vec![BasicBlockData {
            statements: vec![
                Statement {
                    kind: StatementKind::Assign(
                        Place::from_local(LocalId(1)),
                        Rvalue::Use(Operand::ConstantInt(1)),
                    ),
                    source_span: (0, 0),
                },
                Statement {
                    kind: StatementKind::Assign(
                        Place::from_local(LocalId(2)),
                        Rvalue::Ref(BorrowKind::Mut, Place::from_local(LocalId(1))),
                    ),
                    source_span: (0, 0),
                },
                Statement {
                    kind: StatementKind::Assign(
                        Place::from_local(LocalId(3)),
                        Rvalue::Ref(
                            BorrowKind::Mut,
                            Place::from_local(LocalId(2)).project_deref(),
                        ),
                    ),
                    source_span: (0, 0),
                },
                Statement {
                    kind: StatementKind::Assign(
                        Place::from_local(LocalId(2)).project_deref(),
                        Rvalue::Use(Operand::ConstantInt(2)),
                    ),
                    source_span: (0, 0),
                },
                Statement {
                    kind: StatementKind::Assign(
                        Place::from_local(LocalId(4)),
                        Rvalue::Use(Operand::Copy(Place::from_local(LocalId(3)).project_deref())),
                    ),
                    source_span: (0, 0),
                },
            ],
            terminator: Some(terminator(TerminatorKind::Return)),
            ..BasicBlockData::default()
        }];

        assert!(
            body.validate()
                .expect_err("parent access during a mutable reborrow must fail")
                .contains("conflicts with an active reborrow")
        );
    }

    #[test]
    fn shared_references_cannot_be_written_through_in_mir() {
        let mut body = MirBody::new(1);
        body.local_decls = vec![local("_return", "void"), local("shared", "&int")];
        body.basic_blocks = vec![BasicBlockData {
            statements: vec![Statement {
                kind: StatementKind::Assign(
                    Place::from_local(LocalId(1)).project_deref(),
                    Rvalue::Use(Operand::ConstantInt(7)),
                ),
                source_span: (0, 0),
            }],
            terminator: Some(terminator(TerminatorKind::Return)),
            ..BasicBlockData::default()
        }];

        assert!(
            body.validate()
                .expect_err("shared references are read-only")
                .contains("through shared reference")
        );
    }

    #[test]
    fn field_loans_allow_disjoint_fields_and_reject_overlapping_writes() {
        let mut body = MirBody::new(0);
        let mut aggregate = local("pair", "Pair");
        aggregate.field_types = vec!["int".to_owned(), "int".to_owned()];
        body.local_decls = vec![
            local("_return", "void"),
            aggregate,
            local("shared", "&int"),
            local("exclusive", "&mut int"),
            local("observed", "int"),
        ];
        body.basic_blocks = vec![BasicBlockData {
            statements: vec![
                Statement {
                    kind: StatementKind::Assign(
                        Place::from_local(LocalId(1)),
                        Rvalue::Aggregate(vec![Operand::ConstantInt(1), Operand::ConstantInt(2)]),
                    ),
                    source_span: (0, 0),
                },
                Statement {
                    kind: StatementKind::Assign(
                        Place::from_local(LocalId(2)),
                        Rvalue::Ref(
                            BorrowKind::Shared,
                            Place::from_local(LocalId(1)).project_field(FieldIndex(0)),
                        ),
                    ),
                    source_span: (0, 0),
                },
                Statement {
                    kind: StatementKind::Assign(
                        Place::from_local(LocalId(3)),
                        Rvalue::Ref(
                            BorrowKind::Mut,
                            Place::from_local(LocalId(1)).project_field(FieldIndex(1)),
                        ),
                    ),
                    source_span: (0, 0),
                },
                Statement {
                    kind: StatementKind::Assign(
                        Place::from_local(LocalId(1)).project_field(FieldIndex(0)),
                        Rvalue::Use(Operand::ConstantInt(3)),
                    ),
                    source_span: (0, 0),
                },
                Statement {
                    kind: StatementKind::Assign(
                        Place::from_local(LocalId(4)),
                        Rvalue::Use(Operand::Copy(Place::from_local(LocalId(2)).project_deref())),
                    ),
                    source_span: (0, 0),
                },
                Statement {
                    kind: StatementKind::Assign(
                        Place::from_local(LocalId(4)),
                        Rvalue::Use(Operand::Copy(Place::from_local(LocalId(3)).project_deref())),
                    ),
                    source_span: (0, 0),
                },
            ],
            terminator: Some(terminator(TerminatorKind::Return)),
            ..BasicBlockData::default()
        }];

        assert!(
            body.validate()
                .expect_err("writing a borrowed field must fail")
                .contains("conflicts with active Shared borrow")
        );

        body.basic_blocks[0].statements.remove(3);
        body.validate()
            .expect("loans of disjoint fields should not conflict");
    }

    #[test]
    fn partial_moves_preserve_sibling_fields_but_reject_whole_aggregate_reads() {
        let mut body = MirBody::new(0);
        let mut aggregate = local("pair", "Pair");
        aggregate.field_types = vec!["String".to_owned(), "String".to_owned()];
        body.local_decls = vec![
            local("_return", "void"),
            aggregate,
            local("first", "String"),
            local("second", "String"),
        ];
        body.basic_blocks = vec![BasicBlockData {
            statements: vec![
                Statement {
                    kind: StatementKind::Assign(
                        Place::from_local(LocalId(1)),
                        Rvalue::Aggregate(vec![
                            Operand::ConstantString("left".to_owned()),
                            Operand::ConstantString("right".to_owned()),
                        ]),
                    ),
                    source_span: (0, 0),
                },
                Statement {
                    kind: StatementKind::Assign(
                        Place::from_local(LocalId(2)),
                        Rvalue::Use(Operand::Move(
                            Place::from_local(LocalId(1)).project_field(FieldIndex(0)),
                        )),
                    ),
                    source_span: (0, 0),
                },
                Statement {
                    kind: StatementKind::Assign(
                        Place::from_local(LocalId(3)),
                        Rvalue::Use(Operand::Move(
                            Place::from_local(LocalId(1)).project_field(FieldIndex(1)),
                        )),
                    ),
                    source_span: (0, 0),
                },
            ],
            terminator: Some(terminator(TerminatorKind::Return)),
            ..BasicBlockData::default()
        }];
        body.validate()
            .expect("moving both fields independently should be valid");

        body.basic_blocks[0].statements.extend([
            Statement {
                kind: StatementKind::Assign(
                    Place::from_local(LocalId(1)).project_field(FieldIndex(0)),
                    Rvalue::Use(Operand::ConstantString("restored".to_owned())),
                ),
                source_span: (0, 0),
            },
            Statement {
                kind: StatementKind::Assign(
                    Place::from_local(LocalId(2)),
                    Rvalue::Use(Operand::Move(
                        Place::from_local(LocalId(1)).project_field(FieldIndex(0)),
                    )),
                ),
                source_span: (0, 0),
            },
        ]);
        body.validate()
            .expect("a moved field can be reinitialized and used again");

        body.basic_blocks[0].statements.push(Statement {
            kind: StatementKind::Assign(
                Place::from_local(LocalId(2)),
                Rvalue::Use(Operand::Move(Place::from_local(LocalId(1)))),
            ),
            source_span: (0, 0),
        });
        assert!(
            body.validate()
                .expect_err("the whole aggregate is partially moved")
                .contains("partially moved")
        );
    }

    #[test]
    fn calls_reject_duplicate_mutable_reference_arguments() {
        let mut body = MirBody::new(0);
        body.local_decls = vec![
            local("_return", "void"),
            local("value", "int"),
            local("exclusive", "&mut int"),
            local("result", "int"),
        ];
        body.basic_blocks = vec![
            BasicBlockData {
                statements: vec![
                    Statement {
                        kind: StatementKind::Assign(
                            Place::from_local(LocalId(1)),
                            Rvalue::Use(Operand::ConstantInt(1)),
                        ),
                        source_span: (0, 0),
                    },
                    Statement {
                        kind: StatementKind::Assign(
                            Place::from_local(LocalId(2)),
                            Rvalue::Ref(BorrowKind::Mut, Place::from_local(LocalId(1))),
                        ),
                        source_span: (0, 0),
                    },
                ],
                terminator: Some(terminator(TerminatorKind::Call {
                    callee: "takes_two".to_owned(),
                    args: vec![
                        Operand::Move(Place::from_local(LocalId(2))),
                        Operand::Move(Place::from_local(LocalId(2))),
                    ],
                    destination: Place::from_local(LocalId(3)),
                    target: BasicBlockId(1),
                    cleanup: None,
                })),
                ..BasicBlockData::default()
            },
            BasicBlockData {
                terminator: Some(terminator(TerminatorKind::Return)),
                ..BasicBlockData::default()
            },
        ];

        assert!(
            body.validate()
                .expect_err("a mutable reference cannot be passed twice")
                .contains("passes overlapping references")
        );
    }

    #[test]
    fn reference_returns_may_borrow_from_reference_parameters() {
        let mut body = MirBody::new(1);
        body.local_decls = vec![local("_return", "&int"), local("value", "&int")];
        body.basic_blocks = vec![BasicBlockData {
            statements: vec![Statement {
                kind: StatementKind::Assign(
                    MirBody::return_place(),
                    Rvalue::Use(Operand::Copy(Place::from_local(LocalId(1)))),
                ),
                source_span: (0, 0),
            }],
            terminator: Some(terminator(TerminatorKind::Return)),
            ..BasicBlockData::default()
        }];

        body.validate()
            .expect("returning a reference parameter preserves its input region");
    }

    #[test]
    fn validation_rejects_reads_not_initialized_on_every_incoming_path() {
        let value = LocalId(2);
        let mut body = MirBody::new(1);
        body.local_decls = vec![
            local("_return", "int"),
            local("condition", "bool"),
            local("value", "int"),
        ];
        body.basic_blocks = vec![
            BasicBlockData {
                terminator: Some(terminator(TerminatorKind::SwitchInt {
                    discr: Operand::Copy(Place::from_local(LocalId(1))),
                    targets: vec![(1, BasicBlockId(1))],
                    otherwise: BasicBlockId(2),
                })),
                ..BasicBlockData::default()
            },
            BasicBlockData {
                statements: vec![Statement {
                    kind: StatementKind::Assign(
                        Place::from_local(value),
                        Rvalue::Use(Operand::ConstantInt(7)),
                    ),
                    source_span: (0, 0),
                }],
                terminator: Some(terminator(TerminatorKind::Goto {
                    target: BasicBlockId(3),
                })),
                ..BasicBlockData::default()
            },
            BasicBlockData {
                terminator: Some(terminator(TerminatorKind::Goto {
                    target: BasicBlockId(3),
                })),
                ..BasicBlockData::default()
            },
            BasicBlockData {
                statements: vec![Statement {
                    kind: StatementKind::Assign(
                        MirBody::return_place(),
                        Rvalue::Use(Operand::Copy(Place::from_local(value))),
                    ),
                    source_span: (0, 0),
                }],
                terminator: Some(terminator(TerminatorKind::Return)),
                ..BasicBlockData::default()
            },
        ];

        assert!(
            body.validate()
                .expect_err("one incoming path leaves value uninitialized")
                .contains("may be uninitialized")
        );
    }

    #[test]
    fn validation_accepts_return_values_initialized_in_each_branch() {
        let mut body = MirBody::new(1);
        body.local_decls = vec![local("_return", "int"), local("condition", "bool")];
        body.basic_blocks = vec![
            BasicBlockData {
                terminator: Some(terminator(TerminatorKind::SwitchInt {
                    discr: Operand::Copy(Place::from_local(LocalId(1))),
                    targets: vec![(1, BasicBlockId(1))],
                    otherwise: BasicBlockId(2),
                })),
                ..BasicBlockData::default()
            },
            BasicBlockData {
                statements: vec![Statement {
                    kind: StatementKind::Assign(
                        MirBody::return_place(),
                        Rvalue::Use(Operand::ConstantInt(1)),
                    ),
                    source_span: (0, 0),
                }],
                terminator: Some(terminator(TerminatorKind::Return)),
                ..BasicBlockData::default()
            },
            BasicBlockData {
                statements: vec![Statement {
                    kind: StatementKind::Assign(
                        MirBody::return_place(),
                        Rvalue::Use(Operand::ConstantInt(0)),
                    ),
                    source_span: (0, 0),
                }],
                terminator: Some(terminator(TerminatorKind::Return)),
                ..BasicBlockData::default()
            },
        ];

        body.validate()
            .expect("all reachable return paths initialize the return place");
    }

    #[test]
    fn validation_tracks_moves_and_call_results_on_normal_edges_only() {
        let mut moved_body = MirBody::new(1);
        moved_body.local_decls = vec![
            local("_return", "void"),
            local("owned", "String"),
            local("copy", "String"),
        ];
        moved_body.basic_blocks = vec![BasicBlockData {
            statements: vec![
                Statement {
                    kind: StatementKind::Assign(
                        Place::from_local(LocalId(2)),
                        Rvalue::Use(Operand::Move(Place::from_local(LocalId(1)))),
                    ),
                    source_span: (0, 0),
                },
                Statement {
                    kind: StatementKind::Assign(
                        Place::from_local(LocalId(2)),
                        Rvalue::Use(Operand::Copy(Place::from_local(LocalId(1)))),
                    ),
                    source_span: (0, 0),
                },
            ],
            terminator: Some(terminator(TerminatorKind::Return)),
            ..BasicBlockData::default()
        }];
        assert!(
            moved_body
                .validate()
                .expect_err("a moved local cannot be read again")
                .contains("may be uninitialized")
        );

        let mut call_body = MirBody::new(1);
        call_body.local_decls = vec![
            local("_return", "int"),
            local("argument", "int"),
            local("result", "int"),
        ];
        call_body.basic_blocks = vec![
            BasicBlockData {
                terminator: Some(terminator(TerminatorKind::Call {
                    callee: "compute".to_owned(),
                    args: vec![Operand::Copy(Place::from_local(LocalId(1)))],
                    destination: Place::from_local(LocalId(2)),
                    target: BasicBlockId(1),
                    cleanup: Some(BasicBlockId(2)),
                })),
                ..BasicBlockData::default()
            },
            BasicBlockData {
                statements: vec![Statement {
                    kind: StatementKind::Assign(
                        MirBody::return_place(),
                        Rvalue::Use(Operand::Copy(Place::from_local(LocalId(2)))),
                    ),
                    source_span: (0, 0),
                }],
                terminator: Some(terminator(TerminatorKind::Return)),
                ..BasicBlockData::default()
            },
            BasicBlockData {
                terminator: Some(terminator(TerminatorKind::Unreachable)),
                ..BasicBlockData::default()
            },
        ];
        call_body
            .validate()
            .expect("call destination is initialized on the normal successor");
    }
}
