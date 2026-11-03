use nortex_compiler::driver::compile_source_to_mir;

const COUNTER: &str = "public class Counter { public int value = 5; public double scale = 1.5; public Counter(int start) { value += start; } public void bump(int by) { value += by; this.scale = scale * 2.0; } public int get() { return value; } }";

#[cfg(windows)]
fn run(source: &str, name: &str) -> Option<i32> {
    use nortex_compiler::codegen::{CodeGenerator, Target};
    use nortex_compiler::linker::link_object_bytes_with_runtime;
    let compiled = compile_source_to_mir(source);
    assert!(
        !compiled.has_errors(),
        "diagnostics: {:#?}",
        compiled.diagnostics
    );
    let object = CodeGenerator::new(Target::X86_64Windows)
        .emit_object(&compiled.functions)
        .expect("native class MIR should generate COFF");
    let output = std::env::temp_dir().join(format!("nortex-{name}-{}.exe", std::process::id()));
    link_object_bytes_with_runtime(&object, &output).expect("link");
    let code = std::process::Command::new(&output)
        .status()
        .expect("run")
        .code();
    std::fs::remove_file(&output).expect("cleanup");
    code
}

fn errors(source: &str) -> String {
    let compiled = compile_source_to_mir(source);
    assert!(compiled.has_errors(), "expected diagnostics for: {source}");
    format!("{:?}", compiled.diagnostics)
}

#[cfg(windows)]
#[test]
fn fields_constructor_and_methods_run_natively() {
    let source = format!(
        "{COUNTER} public int main() {{ mut Counter c = new Counter(10); c.bump(7); c.value += 1; return c.get(); }}"
    );
    assert_eq!(run(&source, "class-basic"), Some(23));
}

#[cfg(windows)]
#[test]
fn double_fields_and_objects_in_loops_run_natively() {
    let source = format!(
        "{COUNTER} public int main() {{ mut int total = 0; mut int i = 0; while (i < 3) {{ Counter c = new Counter(i); c.bump(1); total += c.get(); i += 1; }} Counter d = new Counter(0); d.bump(0); if (d.scale == 3.0) {{ total += 100; }} return total; }}"
    );
    assert_eq!(run(&source, "class-loop"), Some(121));
}

#[cfg(windows)]
#[test]
fn methods_call_each_other_with_implicit_this() {
    let source = "public class Acc { public int total = 0; public void add(int n) { total += n; } public int addTwice(int n) { add(n); this.add(n); return total; } } public int main() { Acc a = new Acc(); return a.addTwice(21); }";
    assert_eq!(run(source, "class-implicit-this"), Some(42));
}

#[test]
fn class_lowering_produces_constructor_and_this_methods() {
    let compiled = compile_source_to_mir(&format!(
        "{COUNTER} public int main() {{ Counter c = new Counter(1); return c.get(); }}"
    ));
    assert!(!compiled.has_errors(), "{:#?}", compiled.diagnostics);
    let names: Vec<_> = compiled.functions.iter().map(|f| f.name.as_str()).collect();
    for expected in ["Counter.new", "Counter.bump", "Counter.get", "main"] {
        assert!(names.contains(&expected), "{expected} missing in {names:?}");
    }
}

#[test]
fn unsupported_class_features_are_diagnosed() {
    let cases = [
        "public class A { public int x = 0; } public int main() { A a = new A(); return a.y; }",
        "public class A { public int x = 0; } public int main() { A a = new A(); a.nope(); return 0; }",
        "public class A { public int x = 0; } public int main() { A a = new A(1); return 0; }",
    ];
    for case in cases {
        errors(case);
    }
}

#[test]
fn invalid_inheritance_and_override_declarations_are_diagnosed() {
    let cases = [
        "public class Base { public int value() { return 1; } } public class Derived : Base { public override int value() { return 2; } } public int main() { return 0; }",
        "public class Base { public virtual int value() { return 1; } } public class Derived : Base { public int value() { return 2; } } public int main() { return 0; }",
        "public class Base { public Base(int value) {} } public class Derived : Base {} public int main() { Derived item = new Derived(); return 0; }",
        "public class A {} public class B {} public class Derived : A, B {} public int main() { return 0; }",
    ];
    for case in cases {
        errors(case);
    }
}

#[test]
fn object_fields_returns_and_assignment_lower_to_pointer_values() {
    let source = "public class Node { public int value = 42; } public Node make() { return new Node(); } public Node pass(Node node) { return node; } public int main() { mut Node first = make(); Node second = pass(first); first = second; return first.value; }";
    let compiled = compile_source_to_mir(source);
    assert!(!compiled.has_errors(), "{:#?}", compiled.diagnostics);
}

#[cfg(windows)]
#[test]
fn heap_objects_and_string_and_object_fields_run_natively() {
    let source = "public class Node { public int value = 0; public string label = \"ready\"; } public class Holder { public Node child; public Holder(Node value) { child = value; } } public int main() { mut Node first = new Node(); first.value = 19; first.label = \"shared\"; Holder holder = new Holder(first); Node second = holder.child; if (second.label == \"shared\") { return second.value; } return 0; }";
    assert_eq!(run(source, "class-heap-fields"), Some(19));
}

#[cfg(windows)]
#[test]
fn objects_pass_return_and_reassignment_preserve_aliases() {
    let source = "public class Node { public int value = 0; } public Node identity(Node value) { return value; } public Node make() { return new Node(); } public int main() { mut Node first = make(); first.value = 31; mut Node second = identity(first); second.value += 11; first = second; return first.value; }";
    assert_eq!(run(source, "class-heap-alias"), Some(42));
}

#[cfg(windows)]
#[test]
fn cyclic_object_graphs_remain_valid_until_main_returns() {
    let source = "public class Node { public Node next; public int value = 7; } public int main() { mut Node first = new Node(); mut Node second = new Node(); first.next = second; second.next = first; Node next = first.next; Node back = next.next; return back.value; }";
    assert_eq!(run(source, "class-cycle"), Some(7));
}

#[test]
fn generic_classes_are_monomorphized_for_concrete_instantiations() {
    let source = "public class Container<T> { public T value; public Container(T initial) { value = initial; } public T get() { return value; } } public int main() { Container<int> box = new Container<int>(42); return box.get(); }";
    let compiled = compile_source_to_mir(source);
    assert!(!compiled.has_errors(), "{:#?}", compiled.diagnostics);
}

#[cfg(windows)]
#[test]
fn generic_class_methods_run_natively() {
    let source = "public class Container<T> { public T value; public Container(T initial) { value = initial; } public T get() { return value; } } public int main() { Container<int> box = new Container<int>(42); return box.get(); }";
    assert_eq!(run(source, "class-generic"), Some(42));
}

#[test]
fn generic_method_is_monomorphized_for_an_explicit_type_argument() {
    let source = "public class Converter { public T echo<T>(T value) { return value; } } public int main() { Converter converter = new Converter(); return converter.echo<int>(42); }";
    let compiled = compile_source_to_mir(source);
    assert!(!compiled.has_errors(), "{:#?}", compiled.diagnostics);
}

#[cfg(windows)]
#[test]
fn generic_method_specialization_runs_natively() {
    let source = "public class Converter { public T echo<T>(T value) { return value; } } public int main() { Converter converter = new Converter(); return converter.echo<int>(42); }";
    assert_eq!(run(source, "generic-method"), Some(42));
}

#[cfg(windows)]
#[test]
fn generic_string_fields_and_methods_run_natively() {
    let source = "public class Holder<T> { public T value; public Holder(T initial) { value = initial; } public T get() { return value; } } public class Converter { public T echo<T>(T value) { return value; } } public int main() { Holder<string> holder = new Holder<string>(\"Grüße\"); Converter converter = new Converter(); string copy = converter.echo<string>(holder.get()); if (copy == \"Grüße\") { return 43; } return 0; }";
    assert_eq!(run(source, "class-generic-string"), Some(43));
}

#[cfg(windows)]
#[test]
fn inheritance_flattens_fields_and_calls_base_methods() {
    let source = "public class Derived : Base { public int extra = 1; public int sum() { return get() + extra; } } public class Base { public int value = 0; public void set(int n) { value = n; } public int get() { return value; } } public int main() { Derived item = new Derived(); item.set(40); return item.sum() + item.value; }";
    assert_eq!(run(source, "class-inheritance-fields"), Some(81));
}

#[cfg(windows)]
#[test]
fn virtual_calls_dispatch_to_override_using_the_dynamic_type() {
    let source = "public class Base { public virtual int value() { return 1; } } public class Derived : Base { public override int value() { return 42; } } public int main() { Base item = new Derived(); return item.value(); }";
    assert_eq!(run(source, "class-virtual-dispatch"), Some(42));
}

#[cfg(windows)]
#[test]
fn dereferencing_an_uninitialized_object_field_traps() {
    let source = "public class Node { public int value = 7; } public class Holder { public Node child; } public int main() { Holder holder = new Holder(); Node child = holder.child; return child.value; }";
    assert!(run(source, "class-null-trap").is_some_and(|code| code != 0));
}
