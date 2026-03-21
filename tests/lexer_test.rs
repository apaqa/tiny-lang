use tiny_lang::lexer::Lexer;
use tiny_lang::token::Token;

#[test]
fn tokenize_keywords_literals_and_symbols() {
    let source = r#"
        let x = 42;
        fn add(a, b) { return a + b; }
        if true && false || !false { print("ok"); }
    "#;

    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize().unwrap();

    assert_eq!(
        tokens,
        vec![
            Token::Let,
            Token::Ident("x".into()),
            Token::Assign,
            Token::IntLit(42),
            Token::Semicolon,
            Token::Fn,
            Token::Ident("add".into()),
            Token::LParen,
            Token::Ident("a".into()),
            Token::Comma,
            Token::Ident("b".into()),
            Token::RParen,
            Token::LBrace,
            Token::Return,
            Token::Ident("a".into()),
            Token::Plus,
            Token::Ident("b".into()),
            Token::Semicolon,
            Token::RBrace,
            Token::If,
            Token::BoolLit(true),
            Token::And,
            Token::BoolLit(false),
            Token::Or,
            Token::Not,
            Token::BoolLit(false),
            Token::LBrace,
            Token::Print,
            Token::LParen,
            Token::StringLit("ok".into()),
            Token::RParen,
            Token::Semicolon,
            Token::RBrace,
            Token::Eof,
        ]
    );
}

#[test]
fn tokenize_comments_and_comparisons() {
    let source = r#"
        // comment
        let x = 1 == 1;
        let y = x != false;
        let z = 3 <= 4 >= 2 < 9 > 1;
    "#;

    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize().unwrap();

    assert!(tokens.contains(&Token::Eq));
    assert!(tokens.contains(&Token::Ne));
    assert!(tokens.contains(&Token::Le));
    assert!(tokens.contains(&Token::Ge));
    assert!(tokens.contains(&Token::Lt));
    assert!(tokens.contains(&Token::Gt));
}

#[test]
fn tokenize_string_and_number() {
    let source = r#"print("hello"); let value = [12345];"#;
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize().unwrap();

    assert!(tokens.contains(&Token::StringLit("hello".into())));
    assert!(tokens.contains(&Token::IntLit(12345)));
    assert!(tokens.contains(&Token::LBracket));
    assert!(tokens.contains(&Token::RBracket));
}
