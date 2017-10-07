
fn main()
{
    if cfg!(windows)
    {
        println!(r"cargo:rustc-link-search={}\Lib", env!("VULKAN_SDK"));
    }
}