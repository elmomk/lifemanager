Build the Life Manager PWA for production.

Run the following steps:
1. Build Tailwind CSS: `npx @tailwindcss/cli -i ./input.css -o ./assets/main.css --minify`
2. Build the Dioxus app: `dx build --release`
3. Report the output directory and any errors
