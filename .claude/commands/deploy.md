Deploy Life Manager to the Tailscale-served environment.

Steps:
1. Build Tailwind CSS for production: `npx @tailwindcss/cli -i ./input.css -o ./assets/main.css --minify`
2. Build the Dioxus app: `dx build --release`
3. Check if nginx is running with our config: `pgrep -f "life_manager_nginx"`
4. If not running, start nginx: `nginx -c $(pwd)/nginx.conf`
5. Start the production server from the dist directory
6. Verify the app is accessible on port 7000
