fn main() {
    cc::Build::new()
        .file("src/stb/stb_image_impl.c")
        .include("src/stb")
        .compile("stb_image");

    cc::Build::new()
        .file("src/stb/stb_image_resize2_impl.c")
        .include("src/stb")
        .flag("-std=c99")
        .flag("-O2")
        .compile("stb_image_resize2");
}