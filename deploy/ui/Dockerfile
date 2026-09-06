FROM nginx:alpine
COPY index.html login.html style.css zyvor-ux.css zyvor-ux.js app.js guestkit-auth.js guestkit-journey.js guestkit-nebula.js guestkit-console.js guestkit-ai.js guestkit-features.js guestkit-ux.js abyss-tokens.css abyss-components.css /usr/share/nginx/html/
COPY fonts/ /usr/share/nginx/html/fonts/
COPY img/ /usr/share/nginx/html/img/
COPY nginx.conf /etc/nginx/conf.d/default.conf
EXPOSE 80
