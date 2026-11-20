use std::path::Path;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use minisign_verify::{PublicKey, Signature};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let file = args.next().ok_or("missing file path")?;
    let signature_file = args.next().ok_or("missing signature path")?;
    let encoded_public_key = args.next().ok_or("missing updater public key")?;
    if args.next().is_some() {
        return Err("unexpected extra arguments".into());
    }

    let public_key_text = String::from_utf8(
        STANDARD.decode(
            encoded_public_key
                .to_str()
                .ok_or("updater public key is not valid UTF-8")?
                .trim(),
        )?,
    )?;
    let public_key = PublicKey::decode(&public_key_text)?;

    // Tauri stores the complete minisign signature as a base64 string in the
    // .sig file and in latest.json. Decode it exactly like the updater plugin.
    let encoded_signature = std::fs::read_to_string(&signature_file)?;
    let signature_text = String::from_utf8(STANDARD.decode(encoded_signature.trim())?)?;
    let signature = Signature::decode(&signature_text)?;
    let payload = std::fs::read(Path::new(&file))?;
    public_key.verify(&payload, &signature, true)?;

    println!("updater signature matches the configured public key");
    Ok(())
}
