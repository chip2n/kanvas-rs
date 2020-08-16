pub fn create_vertex_module(
    device: &wgpu::Device,
    compiler: &mut shaderc::Compiler,
    src: &str,
    name: &str
) -> shaderc::Result<wgpu::ShaderModule> {
    let spirv = compiler
        .compile_into_spirv(
            src,
            shaderc::ShaderKind::Vertex,
            name,
            "main",
            None,
        )?;
    let data = wgpu::read_spirv(std::io::Cursor::new(spirv.as_binary_u8())).unwrap();
    Ok(device.create_shader_module(&data))
}

pub fn create_fragment_module(
    device: &wgpu::Device,
    compiler: &mut shaderc::Compiler,
    src: &str,
    name: &str
) -> shaderc::Result<wgpu::ShaderModule> {
    let spirv = compiler
        .compile_into_spirv(
            src,
            shaderc::ShaderKind::Fragment,
            name,
            "main",
            None,
        )?;
    let data = wgpu::read_spirv(std::io::Cursor::new(spirv.as_binary_u8())).unwrap();
    Ok(device.create_shader_module(&data))
}
