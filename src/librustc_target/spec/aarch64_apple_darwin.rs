use crate::spec::{LinkerFlavor, Target, TargetOptions, TargetResult};

pub fn target() -> TargetResult {
    let base = super::apple_base::opts();
    let arch = "aarch64";

    Ok(Target {
        llvm_target: "aarch64-apple-macosx11.5.0".to_string(),
        target_endian: "little".to_string(),
        target_pointer_width: "64".to_string(),
        target_c_int_width: "32".to_string(),
        data_layout: "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-f80:128-n8:16:32:64-S128".to_string(),
        arch: arch.to_string(),
        target_os: "macos".to_string(),
        target_env: String::new(),
        target_vendor: "apple".to_string(),
        linker_flavor: LinkerFlavor::Gcc,
        options: TargetOptions {
            features: "+neon,+fp-armv8,+apple-a7".to_string(),
            eliminate_frame_pointer: false,
            max_atomic_width: Some(128),
            abi_blacklist: super::arm_base::abi_blacklist(),
            link_env_remove: vec![ "IPHONEOS_DEPLOYMENT_TARGET".to_string() ],
            target_mcount: "\u{0001}mcount".to_string(),
            ..base
        },
    })
}
