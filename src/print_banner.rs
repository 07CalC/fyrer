use colored::*;

pub fn print_banner() {
    let banner_lines = vec![
        r#" ________                                       "#,
        r#"/        |                                      "#,
        r#"$$$$$$$$/__    __   ______    ______    ______  "#,
        r#"$$ |__  /  |  /  | /      \  /      \  /      \ "#,
        r#"$$    | $$ |  $$ |/$$$$$$  |/$$$$$$  |/$$$$$$  |"#,
        r#"$$$$$/  $$ |  $$ |$$ |  $$/ $$    $$ |$$ |  $$/ "#,
        r#"$$ |    $$ \__$$ |$$ |      $$$$$$$$/ $$ |      "#,
        r#"$$ |    $$    $$ |$$ |      $$       |$$ |      "#,
        r#"$$/      $$$$$$$ |$$/        $$$$$$$/ $$/       "#,
        r#"        /  \__$$ |                              "#,
        r#"        $$    $$/     version: 0.2.3            "#,
        r#"         $$$$$$/      made with <3 by CalC      "#,
    ];

    let max_len = banner_lines.iter().map(|l| l.len()).max().unwrap_or(0);
    let horizontal_border = "─".repeat(max_len + 1);

    println!("\n{}", format!("┌{}┐", horizontal_border).bright_yellow());

    for line in banner_lines {
        println!(
            "{}{} {}{}",
            "│".bright_yellow(),
            line.bright_cyan().bold(),
            "│".bright_yellow(),
            ""
        );
    }

    println!("{}", format!("└{}┘", horizontal_border).bright_yellow());
    println!();
}
