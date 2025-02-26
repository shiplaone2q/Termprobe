//! Koffi layouts and typed asynchronous wrappers come from the C function signatures.
use super::*;

fn ffi_type(ty: &Type) -> Result<String> {
    if matches!(ty, Type::Ptr(_)) {
        return Ok("void *".into());
    }
    Ok(match name(ty)?.as_str() {
        "u8" => "uint8_t",
        "u16" => "uint16_t",
        "u32" => "uint32_t",
        "u64" => "uint64_t",
        "i32" => "int32_t",
        "usize" => "size_t",
        "f64" => "double",
        "bool" => "bool",
        other if other.starts_with("Tui") => return Ok(other.into()),
        other => bail!("unsupported Koffi type: {other}"),
    }
    .into())
}
fn type_reference(ty: &Type) -> Result<String> {
    let ty = ffi_type(ty)?;
    Ok(if ty.starts_with("Tui") {
        format!("types.{ty}")
    } else {
        format!("{ty:?}")
    })
}
fn descriptor(ty: &Type) -> Result<String> {
    if let Type::Ptr(pointer) = ty {
        return Ok(format!("*{}", name(&pointer.elem)?));
    }
    name(ty)
}
fn asynchronous(function: &ItemFn) -> bool {
    matches!(&function.sig.output, ReturnType::Type(_, ty) if matches!(ty.as_ref(), Type::Ptr(_)))
        && !function.sig.ident.to_string().ends_with("_ptr")
}

pub fn generate(
    root: &Path,
    structs: &[&ItemStruct],
    functions: &[&ItemFn],
    version: &str,
    check: bool,
) -> Result<()> {
    let mut js = format!("// {NOTICE}\nimport koffi from 'koffi';\nexport const abiVersion = {version};\nexport const types = {{}};\n");
    let mut layouts = String::from("export const layouts = {\n");
    let mut emitted = BTreeSet::new();
    while emitted.len() < structs.len() {
        let before = emitted.len();
        for item in structs {
            let id = item.ident.to_string();
            if emitted.contains(&id) {
                continue;
            }
            let ready = item.fields.iter().all(|f| {
                !matches!(&f.ty, Type::Path(_))
                    || name(&f.ty).is_ok_and(|n| !n.starts_with("Tui") || emitted.contains(&n))
            });
            if !ready {
                continue;
            }
            writeln!(js, "types.{id} = koffi.struct({{")?;
            writeln!(layouts, "{id}: {{")?;
            for field in &item.fields {
                let field_name = field.ident.as_ref().context("named C field")?.to_string();
                writeln!(js, "{field_name}: {},", type_reference(&field.ty)?)?;
