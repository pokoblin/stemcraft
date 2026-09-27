//! Make sure the model weights are in the download cache and print that path.
//! Used by `script/bundle-macos.sh` to copy the weights into the app bundle.

fn main() -> anyhow::Result<()> {
    stemcraft_core::weights::load(|_, _| {})?;
    println!("{}", stemcraft_core::weights::cache_path()?.display());
    Ok(())
}
