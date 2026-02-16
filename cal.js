// Simple Calculator in JavaScript

function add(a, b) {
    return a + b;
}

function subtract(a, b) {
    return a - b;
}

function multiply(a, b) {
    return a * b;
}

function divide(a, b) {
    if (b === 0) {
        return "Error: Division by zero";
    }
    return a / b;
}

function calculator() {
    console.log("Simple Calculator");
    console.log("Operations: +, -, *, /");
    
    // Example calculations
    console.log("5 + 3 =", add(5, 3));
    console.log("10 - 4 =", subtract(10, 4));
    console.log("6 * 7 =", multiply(6, 7));
    console.log("15 / 3 =", divide(15, 3));
    console.log("10 / 0 =", divide(10, 0)); // Division by zero test
}

// Run the calculator
calculator();