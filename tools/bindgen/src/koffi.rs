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

