use rand::Rng;

pub fn generate_random_string(rng: &mut impl Rng, length: usize) -> String {
    (0..length)
        .map(|_| {
            let idx = rng.gen_range(0..36);
            if idx < 10 {
                (b'0' + idx) as char
            } else {
                (b'a' + idx - 10) as char
            }
        })
        .collect()
}

pub fn create_temp_script(dir: &std::path::Path, rng: &mut impl Rng) {
    let script_output = generate_random_string(rng, 8);
    let script_exit_code: i32 = rng.gen_range(0..5);
    let script_path = dir.join("random_cmd.sh");
    let script_content = format!(
        r#"#!/bin/sh
echo "{}"
pwd
exit {}
"#,
        script_output, script_exit_code
    );
    std::fs::write(&script_path, script_content).unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&script_path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script_path, perms).unwrap();
    }
}
