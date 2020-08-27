#[macro_export]
macro_rules! compile_vertex {
    ($device:expr, $compiler:expr, $path:expr) => {
        crate::shader::create_vertex_module($device, $compiler, include_str!($path), $path);
    }
}

#[macro_export]
macro_rules! compile_frag {
    ($device:expr, $compiler:expr, $path:expr) => {
        crate::shader::create_fragment_module($device, $compiler, include_str!($path), $path);
    }
}

pub fn create_vertex_module(
    device: &wgpu::Device,
    compiler: &mut shaderc::Compiler,
    src: &str,
    name: &str,
) -> shaderc::Result<wgpu::ShaderModule> {
    let spirv =
        compiler.compile_into_spirv(src, shaderc::ShaderKind::Vertex, name, "main", None)?;
    let data = wgpu::util::make_spirv(spirv.as_binary_u8());
    Ok(device.create_shader_module(data))
}

pub fn create_fragment_module(
    device: &wgpu::Device,
    compiler: &mut shaderc::Compiler,
    src: &str,
    name: &str,
) -> shaderc::Result<wgpu::ShaderModule> {
    let spirv =
        compiler.compile_into_spirv(src, shaderc::ShaderKind::Fragment, name, "main", None)?;
    let data = wgpu::util::make_spirv(spirv.as_binary_u8());
    Ok(device.create_shader_module(data))
}
