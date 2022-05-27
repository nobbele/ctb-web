import { defineNuxtConfig } from 'nuxt'

// https://v3.nuxtjs.org/api/configuration/nuxt.config
export default defineNuxtConfig({
    typescript: {
        strict: true,
        typeCheck: true,
    },
    css: ["@/assets/reset.css"],
    build: {
        loaders: {
            sass: {
                sassOptions: {
                    indentedSyntax: true
                }
            },
        },
    },
});
