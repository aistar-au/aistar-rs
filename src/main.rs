use std::io;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Operator {
    Add,
    Subtract,
    Multiply,
    Divide,
}

impl Operator {
    fn apply(&self, left: f64, right: f64) -> f64 {
        match self {
            Operator::Add => left + right,
            Operator::Subtract => left - right,
            Operator::Multiply => left * right,
            Operator::Divide => left / right,
            Operator::Radical => left.sqrt(),
        }
    }
}

pub struct Calculator {
    result: f64,
}

impl Calculator {
    pub fn new() -> Self {
        Calculator { result: 0.0 }
    }

    pub fn calculate(&mut self, left: f64, operator: Operator, right: f64) -> f64 {
        let result = operator.apply(left, right);
        self.result = result;
        result
    }

    pub fn get_result(&self) -> f64 {
        self.result
    }

    pub fn clear(&mut self) {
        self.result = 0.0;
    }
}

pub fn parse_operator(input: &str) -> Option<Operator> {
    match input.trim() {
        "+" => Some(Operator::Add),
        "-" => Some(Operator::Subtract),
        "*" => Some(Operator::Multiply),
        "/" => Some(Operator::Divide),
        "rad" => Some(Operator::Radical),
        _ => None,
    }
}

pub fn parse_number(input: &str) -> Result<f64, std::num::ParseFloatError> {
    input.trim().parse::<f64>()
}

fn main() {
    let mut calc = Calculator::new();

    println!("Radical Calculator");
    println!("Enter 'quit' to exit");
    println!("Enter operations like: 16 rad 0 (to get square root of 16)");

    loop {
        println!("Enter operation:");
        let mut input = String::new();
        io::stdin()
            .read_line(&mut input)
            .expect("Failed to read input");

        let input = input.trim();
        if input == "quit" {
            break;
        }

        let parts: Vec<&str> = input.split_whitespace().collect();
        if parts.len() != 3 {
            println!("Invalid input. Please enter: number operator number");
            continue;
        }

        let left = parse_number(parts[0]);
        let operator = parse_operator(parts[1]);
        let right = parse_number(parts[2]);

        match (left, operator, right) {
            (Ok(left_val), Some(op), Ok(right_val)) => {
                let result = calc.calculate(left_val, op, right_val);
                println!("Result: {}", result);
            }
            _ => {
                println!("Invalid input. Please check your numbers and operator.");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculator_operations() {
        let mut calc = Calculator::new();

        // Test addition
        assert_eq!(calc.calculate(2.0, Operator::Add, 3.0), 5.0);
        assert_eq!(calc.get_result(), 5.0);

        // Test subtraction
        assert_eq!(calc.calculate(10.0, Operator::Subtract, 4.0), 6.0);
        assert_eq!(calc.get_result(), 6.0);

        // Test multiplication
        assert_eq!(calc.calculate(3.0, Operator::Multiply, 7.0), 21.0);
        assert_eq!(calc.get_result(), 21.0);

        // Test division
        assert_eq!(calc.calculate(15.0, Operator::Divide, 3.0), 5.0);
        assert_eq!(calc.get_result(), 5.0);

        // Test radical
        assert_eq!(calc.calculate(16.0, Operator::Radical, 0.0), 4.0);
        assert_eq!(calc.get_result(), 4.0);
    }

    #[test]
    fn test_parse_operator() {
        assert_eq!(parse_operator("+"), Some(Operator::Add));
        assert_eq!(parse_operator("-"), Some(Operator::Subtract));
        assert_eq!(parse_operator("*"), Some(Operator::Multiply));
        assert_eq!(parse_operator("/"), Some(Operator::Divide));
        assert_eq!(parse_operator("invalid"), None);
    }
}
