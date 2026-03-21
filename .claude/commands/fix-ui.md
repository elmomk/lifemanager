Fix a UI issue in the Life Manager app.

Arguments: $ARGUMENTS (description of the UI issue)

Steps:
1. Identify which page/component is affected based on the description
2. Read the relevant component and page files
3. Read `input.css` for custom Tailwind theme values if styling is involved
4. Make the fix, keeping the glassmorphism design language and dark mode support
5. Run `cargo check` to verify the fix compiles
