//! Janela de Ajuda: como conectar o Showtime ao GrandMA2.
//!
//! Guia passo a passo exibido dentro do app (menu Ajuda). A UI é em português.

/// Renderiza a janela de ajuda. `open` controla a visibilidade (o usuário
/// pode fechar pelo X); quando fechada, o app para de exibi-la.
pub fn show(ctx: &egui::Context, open: &mut bool) {
    let mut open_local = *open;
    egui::Window::new("Ajuda — Conexão GrandMA2")
        .open(&mut open_local)
        .default_size([560.0, 480.0])
        .resizable(true)
        .show(ctx, |ui| {
            ui.heading("Como conectar ao console GrandMA2");
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(
                    "O Showtime envia comandos ao MA2 pela rede (TCP). O protocolo é \
                     não-oficial e depende do console aceitar a conexão.",
                )
                .weak(),
            );
            ui.add_space(8.0);
            ui.separator();

            steps(ui);
            ui.add_space(8.0);
            ui.separator();
            commands(ui);
            ui.add_space(8.0);
            ui.separator();
            troubleshooting(ui);
        });
    *open = open_local;
}

fn step(ui: &mut egui::Ui, n: u32, title: &str, detail: &str) {
    ui.horizontal_wrapped(|ui| {
        ui.label(egui::RichText::new(format!("{n}.")).strong().color(egui::Color32::from_rgb(240, 200, 60)));
        ui.vertical(|ui| {
            ui.label(egui::RichText::new(title).strong());
            ui.label(egui::RichText::new(detail).weak());
        });
    });
    ui.add_space(6.0);
}

fn steps(ui: &mut egui::Ui) {
    ui.heading("Passo a passo");
    ui.add_space(4.0);

    step(
        ui,
        1,
        "Ligue o console e conecte na mesma rede",
        "O GrandMA2 (ou onPC) e o computador com o Showtime precisam estar na mesma \
         rede (cabo direto ou switch). Confira os cabos e o LED de link.",
    );
    step(
        ui,
        2,
        "Descubra o IP do console",
        "No console, abra Setup → Network (ou na tela do sistema) e anote o endereço IP \
         (ex.: 192.168.1.10). No onPC, use o IP da máquina (ipconfig no Windows).",
    );
    step(
        ui,
        3,
        "Configure IP e porta no Showtime",
        "Menu Configurações → seção \"Rede GrandMA2\". Informe o IP do console e a porta \
         (padrão: 3000). Salve/feche a janela.",
    );
    step(
        ui,
        4,
        "Conecte",
        "Na janela de Configurações, clique em \"Conectar\". O status deve mudar para \
         \"conectado\". Também é possível abrir essa janela pelo menu Ao vivo → \
         \"Conectar MA2 (TCP)...\".",
    );
    step(
        ui,
        5,
        "Teste",
        "Toque a música e cruze um marcador: o console deve executar o comando \
         (ex.: Go Executor 1). Se não disparar, veja a seção de problemas abaixo.",
    );
}

fn commands(ui: &mut egui::Ui) {
    ui.heading("Comandos enviados");
    ui.add_space(4.0);
    ui.label("Cada marcador vira um comando por linha (terminado com \\n):");
    ui.add_space(4.0);
    for (label, desc) in [
        ("Go", "Go Executor N — dispara o executor"),
        ("Pause", "Pause Executor N — pausa"),
        ("Goto", "Goto Cue N Executor M — pula para a cue"),
        ("Toggle / Load", "Go Executor N — mesmo comando de Go"),
    ] {
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(format!("{label}:")).strong());
            ui.label(desc);
        });
    }
}

fn troubleshooting(ui: &mut egui::Ui) {
    ui.heading("Se a conexão não funcionar");
    ui.add_space(4.0);

    for (problem, fix) in [
        (
            "\"timeout ao conectar\"",
            "IP errado ou console desligado/em outra rede. Confirme o IP no Setup → \
             Network e teste com ping (prompt: ping <ip>).",
        ),
        (
            "\"endereço inválido\"",
            "IP ou porta digitados incorretamente. Formato do IP: 192.168.1.10.",
        ),
        (
            "Conecta, mas nada dispara",
            "O MA2 pode exigir permissão para comandos de rede, ou o firewall do \
             computador/console bloqueia a porta 3000. Verifique a configuração de \
             rede do console e libere a porta no firewall.",
        ),
        (
            "Desconectou no meio do show",
            "Cabo/switch instável ou console reiniciado. Clique em \"Conectar\" \
             novamente (a conexão não reconecta sozinha).",
        ),
        (
            "Status diz conectado, mas console não responde",
            "TCP aceitou a conexão, mas o console pode estar ignorando comandos. \
             Teste no console com um comando manual equivalente (ex.: \"Go Executor 1\" \
             na linha de comando) para confirmar que a cue existe.",
        ),
    ] {
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(format!("• {problem}:")).strong());
            ui.label(fix);
        });
        ui.add_space(2.0);
    }

    ui.add_space(6.0);
    ui.label(
        egui::RichText::new(
            "Importante: o protocolo TCP do MA2 não é documentado oficialmente pela MA \
             Lighting — o funcionamento pode variar entre versões do software do console.",
        )
        .weak(),
    );
}