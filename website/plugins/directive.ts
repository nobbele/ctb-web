import { defineNuxtPlugin } from '#app'

export default defineNuxtPlugin(app => {
    app.vueApp.directive('click-outside', {
        mounted: function (el, binding, _vnode) {
            el.clickOutsideEvent = function (event: MouseEvent) {
                const isClickOutside = event.target !== el && !el.contains(event.target);
                if (isClickOutside) {
                    binding.value(event, el);
                }
            };
            document.body.addEventListener('click', el.clickOutsideEvent)
        },
        unmounted: function (el) {
            document.body.removeEventListener('click', el.clickOutsideEvent)
        },
    });
})