use super::lexer::Lexer;

enum Instruction<'src> {
    ChangeSection(&'src str),
}
