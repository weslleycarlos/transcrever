//! Utilitarios compartilhados.

/// Aplica, no Windows, a flag CREATE_NO_WINDOW para que processos de console
/// (whisper-cli, ffmpeg, python) nao abram uma janela preta de cmd durante a
/// transcricao. Em outras plataformas e um no-op.
pub fn no_window(cmd: &mut std::process::Command) -> &mut std::process::Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}
