// Example HTTP server configuration for optimal caching
// This would be used with actix-web, axum, or similar

use std::time::Duration;

// Cache control headers for different asset types
pub fn get_cache_headers(path: &str) -> (&'static str, &'static str) {
    match path {
        // WASM files - long cache with immutable (content-hashed by trunk)
        p if p.ends_with(".wasm") => (
            "Cache-Control", 
            "public, max-age=31536000, immutable"
        ),
        // JS files - long cache with immutable (content-hashed by trunk)  
        p if p.ends_with(".js") => (
            "Cache-Control", 
            "public, max-age=31536000, immutable"
        ),
        // CSS files - long cache with immutable
        p if p.ends_with(".css") => (
            "Cache-Control", 
            "public, max-age=31536000, immutable"
        ),
        // Font files - very long cache
        p if p.ends_with(".woff2") || p.ends_with(".woff") || p.ends_with(".ttf") => (
            "Cache-Control", 
            "public, max-age=31536000, immutable"
        ),
        // HTML files - short cache to allow updates
        p if p.ends_with(".html") => (
            "Cache-Control", 
            "public, max-age=3600, must-revalidate"
        ),
        // Default for other assets
        _ => (
            "Cache-Control", 
            "public, max-age=86400"
        ),
    }
}

// Compression configuration
pub fn should_compress(path: &str) -> bool {
    matches!(
        path.split('.').last(),
        Some("js") | Some("css") | Some("html") | Some("json") | Some("wasm")
    )
}

// Security headers for the app
pub fn get_security_headers() -> Vec<(&'static str, &'static str)> {
    vec![
        ("X-Content-Type-Options", "nosniff"),
        ("X-Frame-Options", "DENY"),
        ("X-XSS-Protection", "1; mode=block"),
        ("Referrer-Policy", "strict-origin-when-cross-origin"),
        // CSP for WebGPU apps
        ("Content-Security-Policy", 
         "default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; connect-src 'self'; worker-src 'self'"),
    ]
}

// Example configuration for nginx
pub fn generate_nginx_config() -> String {
    r#"
# Nginx configuration for optimized WASM serving
server {
    listen 80;
    server_name your-domain.com;
    root /var/www/html;
    
    # Compression
    gzip on;
    gzip_vary on;
    gzip_min_length 1024;
    gzip_types
        application/wasm
        application/javascript
        text/css
        text/html
        application/json;
    
    # Cache static assets with hash
    location ~* \.(wasm|js|css)$ {
        expires 1y;
        add_header Cache-Control "public, immutable";
        add_header Vary Accept-Encoding;
    }
    
    # Cache fonts
    location ~* \.(woff2|woff|ttf)$ {
        expires 1y;
        add_header Cache-Control "public, immutable";
    }
    
    # HTML with short cache
    location ~* \.html$ {
        expires 1h;
        add_header Cache-Control "public, must-revalidate";
    }
    
    # Security headers
    add_header X-Content-Type-Options nosniff;
    add_header X-Frame-Options DENY;
    add_header X-XSS-Protection "1; mode=block";
    add_header Referrer-Policy "strict-origin-when-cross-origin";
    
    # CSP for WebGPU
    add_header Content-Security-Policy "default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; connect-src 'self'; worker-src 'self'";
    
    # WASM MIME type
    location ~* \.wasm$ {
        add_header Content-Type application/wasm;
    }
    
    # Fallback for SPA
    location / {
        try_files $uri $uri/ /index.html;
    }
}
"#.to_string()
}

// Example for CDN/Cloudflare configuration
pub fn generate_cloudflare_rules() -> String {
    r#"
# Cloudflare Page Rules for optimal caching

# Rule 1: Cache static assets
URL: your-domain.com/*.wasm
Cache Level: Cache Everything
Edge Cache TTL: 1 month
Browser Cache TTL: 1 month

# Rule 2: Cache JS/CSS
URL: your-domain.com/*.js
Cache Level: Cache Everything
Edge Cache TTL: 1 month
Browser Cache TTL: 1 month

# Rule 3: HTML with revalidation
URL: your-domain.com/*.html
Cache Level: Cache Everything
Edge Cache TTL: 1 hour
Browser Cache TTL: 1 hour

# Security Headers (via Transform Rules)
- X-Content-Type-Options: nosniff
- X-Frame-Options: DENY
- X-XSS-Protection: 1; mode=block
- Content-Security-Policy: default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'
"#.to_string()
}
"#.to_string()
}