
fn main()
{
    if cfg!(windows)
    {
        println!(r"cargo:rustc-link-search={}\Lib", std::env::var("VULKAN_SDK").unwrap());
    }
}