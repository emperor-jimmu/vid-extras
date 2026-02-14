---
inclusion: always
---
<!------------------------------------------------------------------------------------
   Add rules to this file or a short description and have Kiro refine them for you.
   
   Learn about inclusion modes: https://kiro.dev/docs/steering/#inclusion-modes
-------------------------------------------------------------------------------------> 
# Code Quality Steering Document for Rust Projects

## Purpose

This document establishes the code quality standards and design principles for this Rust project. The goal is to create maintainable, extendable, and scalable software that remains manageable as requirements change and the application grows. [rusting.substack](https://rusting.substack.com/p/solid-rust)

## SOLID Principles in Rust

### Single Responsibility Principle (SRP)
Each module, struct, function, or trait should have one, and only one, reason to change. [codesignal](https://codesignal.com/learn/courses/applying-clean-code-principles-in-rust/lessons/applying-clean-code-principles-in-rust-understanding-and-implementing-solid-principles)

**Implementation Guidelines:**
- Keep modules focused on a single task or domain concept
- Leverage Rust's ownership and borrowing rules to naturally promote smaller, focused units of functionality [holeoftherabbit](https://www.holeoftherabbit.com/2025/01/19/rust-and-the-solid-principals/)
- Favor composition through structs, enums, and modules rather than monolithic structures
- Each function should perform one well-defined operation
- Separate concerns: logging, file I/O, business logic, and data processing should live in distinct components [darrenhorrocks.co](https://www.darrenhorrocks.co.uk/solid-principles-rust-with-examples/)

### Open/Closed Principle (OCP)
Software entities should be open for extension but closed for modification. [codesignal](https://codesignal.com/learn/courses/applying-clean-code-principles-in-rust/lessons/applying-clean-code-principles-in-rust-understanding-and-implementing-solid-principles)

**Implementation Guidelines:**
- Use traits and trait implementations to extend functionality without modifying existing code [codesignal](https://codesignal.com/learn/courses/applying-clean-code-principles-in-rust/lessons/applying-clean-code-principles-in-rust-understanding-and-implementing-solid-principles)
- Leverage enum-based polymorphism for extending behavior
- Design trait boundaries that allow new implementations without changing core logic
- Utilize generics with trait bounds to create extensible APIs

### Liskov Substitution Principle (LSP)
Implementations should honor the contracts of their traits, ensuring substitutability. [holeoftherabbit](https://www.holeoftherabbit.com/2025/01/19/rust-and-the-solid-principals/)

**Implementation Guidelines:**
- Rust avoids classical inheritance, so LSP is achieved through traits and polymorphism [holeoftherabbit](https://www.holeoftherabbit.com/2025/01/19/rust-and-the-solid-principals/)
- Ensure trait implementations maintain expected behavior and invariants
- Leverage Rust's type system and compile-time checks to catch substitution errors early [holeoftherabbit](https://www.holeoftherabbit.com/2025/01/19/rust-and-the-solid-principals/)
- Document trait contracts clearly, including preconditions and postconditions

### Interface Segregation Principle (ISP)
Design small, focused traits to avoid forcing implementations to depend on unused interfaces. [codesignal](https://codesignal.com/learn/courses/applying-clean-code-principles-in-rust/lessons/applying-clean-code-principles-in-rust-understanding-and-implementing-solid-principles)

**Implementation Guidelines:**
- Create granular traits rather than large, monolithic trait definitions
- Split traits into smaller, cohesive units when they serve multiple purposes
- Allow types to implement only the traits they need
- Use trait composition to build complex behaviors from simple traits

### Dependency Inversion Principle (DIP)
Depend on abstractions (traits) rather than concrete implementations to decouple high-level and low-level modules. [codesignal](https://codesignal.com/learn/courses/applying-clean-code-principles-in-rust/lessons/applying-clean-code-principles-in-rust-understanding-and-implementing-solid-principles)

**Implementation Guidelines:**
- Use traits to define interfaces between modules
- Accept trait objects or generic trait bounds in function signatures
- Avoid tight coupling to concrete types in business logic
- Enable dependency injection through trait-based abstractions

## Code Quality Standards

### Readability
Code should be self-explanatory and follow Rust conventions. [dev](https://dev.to/mbayoun95/rust-clean-code-crafting-elegant-efficient-and-maintainable-software-27ce)

**Requirements:**
- Use meaningful, descriptive names for functions, variables, types, and modules
- Follow consistent formatting using `rustfmt`
- Maintain clear code structure and logical organization
- Write self-documenting code that minimizes the need for explanatory comments
- Add comments only to explain *why* something is done, not *what* is being done
- Document public APIs with rustdoc comments (`///`)

### Simplicity
Avoid unnecessary complexity and write code that does one thing well. [dev](https://dev.to/mbayoun95/rust-clean-code-crafting-elegant-efficient-and-maintainable-software-27ce)

**Requirements:**
- Prefer straightforward solutions over clever ones
- Break complex functions into smaller, composable pieces
- Use Rust idioms rather than porting patterns from other languages
- Avoid premature optimization
- Choose appropriate data structures for the use case

### Encapsulation
Hide implementation details and expose only necessary interfaces. [holeoftherabbit](https://www.holeoftherabbit.com/2025/01/19/rust-and-the-solid-principals/)

**Requirements:**
- Use appropriate visibility modifiers (`pub`, `pub(crate)`, private by default)
- Keep internal implementation details private
- Expose minimal public APIs that provide clear contracts
- Use the builder pattern for complex object construction
- Leverage newtype patterns to enforce type safety and hide representation

## Code Smells and Anti-Patterns

### Bloaters
Avoid code, methods, and structs that grow to unmanageable sizes. [refactoring](https://refactoring.guru/refactoring/smells)

**Prohibited:**
- Functions longer than 50-100 lines (excluding tests)
- Structs with more than 10-12 fields without clear justification
- Modules exceeding 500 lines without logical subdivision
- Deeply nested control flow (more than 3-4 levels)

### Rust-Specific Anti-Patterns

**Excessive Cloning:**
- Avoid unnecessary `.clone()` calls [mcpmarket](https://mcpmarket.com/tools/skills/rust-anti-pattern-refactor)
- Use borrowing and references appropriately
- Consider `Rc`/`Arc` for shared ownership when cloning is unavoidable

**Unsafe Unwrap Usage:**
- Never use `.unwrap()` in production code [reddit](https://www.reddit.com/r/rust/comments/14lz0rz/best_practices_for_rust/)
- Use `.expect()` with descriptive messages for truly impossible failure cases [reddit](https://www.reddit.com/r/rust/comments/14lz0rz/best_practices_for_rust/)
- Prefer pattern matching, `if let`, or the `?` operator for error handling
- Leverage `Option::flatten()` and `Result::flatten()` for nested types [reddit](https://www.reddit.com/r/rust/comments/14lz0rz/best_practices_for_rust/)

**Large Structs with Self-Referential Methods:**
- Avoid designs where methods produce references and then mutate `self` [jrasky.github](https://jrasky.github.io/posts/2018-02-17-two-anti-patterns-rust/)
- Refactor into smaller, focused structs
- Consider splitting state from behavior

**Fighting the Borrow Checker:**
- Don't work around borrow checker with excessive cloning or unsafe code
- Redesign data structures to work with Rust's ownership model
- Consider interior mutability patterns (`RefCell`, `Mutex`) when truly needed

## Dead Code Elimination

**Requirements:**
- Run `cargo check` and `cargo clippy` regularly [github](https://github.com/ZhangHanDong/rust-code-review-guidelines)
- Address all dead code warnings before committing
- Remove commented-out code; rely on version control instead
- Prune unused dependencies from `Cargo.toml`
- Use `cargo-udeps` to identify unnecessary dependencies

## Error Handling

**Standards:**
- Implement proper error handling throughout the codebase [github](https://github.com/ZhangHanDong/rust-code-review-guidelines)
- Use `Result<T, E>` for operations that can fail
- Define custom error types using `thiserror` or similar
- Propagate errors using the `?` operator
- Log errors at appropriate levels with context [github](https://github.com/ZhangHanDong/rust-code-review-guidelines)
- Provide clear, specific error messages that help locate issues [github](https://github.com/ZhangHanDong/rust-code-review-guidelines)

## Safety and Correctness

**Requirements:**
- Ensure code compiles without warnings [github](https://github.com/ZhangHanDong/rust-code-review-guidelines)
- Document all `unsafe` code with safety invariants and justification [github](https://github.com/ZhangHanDong/rust-code-review-guidelines)
- Minimize use of `unsafe` blocks; use only when absolutely necessary
- Verify business logic for edge cases and error conditions [github](https://github.com/ZhangHanDong/rust-code-review-guidelines)
- Use assertions at key points to verify assumptions [github](https://github.com/ZhangHanDong/rust-code-review-guidelines)
- Leverage Rust's type system to prevent bugs at compile time

## Testing and Maintainability

**Standards:**
- Write unit tests for all public APIs and critical logic
- Follow the "working code, then tests, then refactor" approach [reddit](https://www.reddit.com/r/rust/comments/14lz0rz/best_practices_for_rust/)
- Make code independently testable through dependency injection
- Use integration tests for end-to-end validation
- Maintain test coverage for refactored code

## Performance Considerations

**Guidelines:**
- Identify obvious inefficiencies like unnecessary allocations [github](https://github.com/ZhangHanDong/rust-code-review-guidelines)
- Use appropriate data structures and algorithms for the use case
- Consider lazy evaluation, async processing, or parallelism where beneficial [github](https://github.com/ZhangHanDong/rust-code-review-guidelines)
- Benchmark before optimizing to prevent performance regression [github](https://github.com/ZhangHanDong/rust-code-review-guidelines)
- Balance compilation size, compilation time, and runtime performance [github](https://github.com/ZhangHanDong/rust-code-review-guidelines)

## Continuous Improvement

**Process:**
- Run `cargo clippy` with strict lints enabled
- Use `cargo fmt` to maintain consistent formatting
- Conduct code reviews focusing on design principles, not just syntax
- Regularly refactor code to improve quality
- Keep build times fast by running `cargo check` frequently [reddit](https://www.reddit.com/r/rust/comments/14lz0rz/best_practices_for_rust/)
- Document architectural decisions and design patterns used

## Enforcement

All code must pass the following checks before merging:
1. `cargo build` - compiles without errors or warnings
2. `cargo test` - all tests pass
3. `cargo clippy -- -D warnings` - no clippy warnings
4. `cargo fmt -- --check` - code is properly formatted
5. Code review approval confirming adherence to this document
