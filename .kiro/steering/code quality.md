---
inclusion: always
---

# Code Quality Steering Document for Rust Projects

## Purpose

This document establishes the code quality standards and design principles for this Rust project. The goal is to create maintainable, extendable, and scalable software that remains manageable as requirements change and the application grows. [rusting.substack](https://rusting.substack.com/p/solid-rust)

## SOLID Principles in Rust

### Single Responsibility Principle (SRP)

Each module, struct, function, or trait should have one, and only one, reason to change.

**Implementation Guidelines:**

- Keep modules focused on a single task or domain concept
- Leverage Rust's ownership and borrowing rules to naturally promote smaller, focused units of functionality
- Favor composition through structs, enums, and modules rather than monolithic structures
- Each function should perform one well-defined operation
- Separate concerns: logging, file I/O, business logic, and data processing should live in distinct components

### Open/Closed Principle (OCP)

Software entities should be open for extension but closed for modification.

**Implementation Guidelines:**

- Use traits and trait implementations to extend functionality without modifying existing code
- Leverage enum-based polymorphism for extending behavior
- Design trait boundaries that allow new implementations without changing core logic
- Utilize generics with trait bounds to create extensible APIs

### Liskov Substitution Principle (LSP)

Implementations should honor the contracts of their traits, ensuring substitutability.

**Implementation Guidelines:**

- Rust avoids classical inheritance, so LSP is achieved through traits and polymorphism
- Ensure trait implementations maintain expected behavior and invariants
- Leverage Rust's type system and compile-time checks to catch substitution errors early
- Document trait contracts clearly, including preconditions and postconditions

### Interface Segregation Principle (ISP)

Design small, focused traits to avoid forcing implementations to depend on unused interfaces.

**Implementation Guidelines:**

- Create granular traits rather than large, monolithic trait definitions
- Split traits into smaller, cohesive units when they serve multiple purposes
- Allow types to implement only the traits they need
- Use trait composition to build complex behaviors from simple traits

### Dependency Inversion Principle (DIP)

Depend on abstractions (traits) rather than concrete implementations to decouple high-level and low-level modules.

**Implementation Guidelines:**

- Use traits to define interfaces between modules
- Accept trait objects or generic trait bounds in function signatures
- Avoid tight coupling to concrete types in business logic
- Enable dependency injection through trait-based abstractions

## Code Quality Standards

### Readability

Code should be self-explanatory and follow Rust conventions.

**Requirements:**

- Use meaningful, descriptive names for functions, variables, types, and modules
- Follow consistent formatting using `rustfmt`
- Maintain clear code structure and logical organization
- Write self-documenting code that minimizes the need for explanatory comments
- Add comments only to explain _why_ something is done, not _what_ is being done
- Document public APIs with rustdoc comments (`///`)

### Simplicity

Avoid unnecessary complexity and write code that does one thing well.

**Requirements:**

- Prefer straightforward solutions over clever ones
- Break complex functions into smaller, composable pieces
- Use Rust idioms rather than porting patterns from other languages
- Avoid premature optimization
- Choose appropriate data structures for the use case

### Encapsulation

Hide implementation details and expose only necessary interfaces.

**Requirements:**

- Use appropriate visibility modifiers (`pub`, `pub(crate)`, private by default)
- Keep internal implementation details private
- Expose minimal public APIs that provide clear contracts
- Use the builder pattern for complex object construction
- Leverage newtype patterns to enforce type safety and hide representation

## Code Smells and Anti-Patterns

### Bloaters

Avoid code, methods, and structs that grow to unmanageable sizes.

**Prohibited:**

- Functions longer than 50-100 lines (excluding tests)
- Structs with more than 10-12 fields without clear justification
- Modules exceeding 500 lines without logical subdivision
- Deeply nested control flow (more than 3-4 levels)

### Rust-Specific Anti-Patterns

**Excessive Cloning:**

- Avoid unnecessary `.clone()` calls
- Use borrowing and references appropriately
- Consider `Rc`/`Arc` for shared ownership when cloning is unavoidable

**Unsafe Unwrap Usage:**

- Never use `.unwrap()` in production code
- Use `.expect()` with descriptive messages for truly impossible failure cases
- Prefer pattern matching, `if let`, or the `?` operator for error handling
- Leverage `Option::flatten()` and `Result::flatten()` for nested types

**Large Structs with Self-Referential Methods:**

- Avoid designs where methods produce references and then mutate `self`
- Refactor into smaller, focused structs
- Consider splitting state from behavior

**Fighting the Borrow Checker:**

- Don't work around borrow checker with excessive cloning or unsafe code
- Redesign data structures to work with Rust's ownership model
- Consider interior mutability patterns (`RefCell`, `Mutex`) when truly needed

## Dead Code Elimination

**Requirements:**

- Run `cargo check` and `cargo clippy` regularly
- Address all dead code warnings before committing
- Remove commented-out code; rely on version control instead
- Prune unused dependencies from `Cargo.toml`
- Use `cargo-udeps` to identify unnecessary dependencies

## Error Handling

**Standards:**

- Implement proper error handling throughout the codebase
- Use `Result<T, E>` for operations that can fail
- Define custom error types using `thiserror` or similar
- Propagate errors using the `?` operator
- Log errors at appropriate levels with context
- Provide clear, specific error messages that help locate issues

## Safety and Correctness

**Requirements:**

- Ensure code compiles without warnings
- Document all `unsafe` code with safety invariants and justification
- Minimize use of `unsafe` blocks; use only when absolutely necessary
- Verify business logic for edge cases and error conditions
- Use assertions at key points to verify assumptions
- Leverage Rust's type system to prevent bugs at compile time

## Testing and Maintainability

**Standards:**

- Write unit tests for all public APIs and critical logic
- Follow the "working code, then tests, then refactor" approach
- Make code independently testable through dependency injection
- Use integration tests for end-to-end validation
- Maintain test coverage for refactored code

## Performance Considerations

**Guidelines:**

- Identify obvious inefficiencies like unnecessary allocations
- Use appropriate data structures and algorithms for the use case
- Consider lazy evaluation, async processing, or parallelism where beneficial
- Benchmark before optimizing to prevent performance regression
- Balance compilation size, compilation time, and runtime performance

## Continuous Improvement

**Process:**

- Run `cargo clippy` with strict lints enabled
- Use `cargo fmt` to maintain consistent formatting
- Conduct code reviews focusing on design principles, not just syntax
- Regularly refactor code to improve quality
- Keep build times fast by running `cargo check` frequently
- Document architectural decisions and design patterns used

## Enforcement

All code must pass the following checks before merging:

1. `cargo build` - compiles without errors or warnings
2. `cargo test` - all tests pass
3. `cargo clippy -- -D warnings` - no clippy warnings
4. `cargo fmt -- --check` - code is properly formatted
5. Code review approval confirming adherence to this document
