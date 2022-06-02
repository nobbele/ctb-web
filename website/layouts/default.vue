<template>
    <div class="page-wrapper">
        <Header />
        <div class="seperator" />
        <transition-group name="notification-list" tag="ul" class="notification-wrapper">
            <li v-for="notification in activeNotifications" class="notification" :key="notification.id">
                <notification :message="notification.message" :type="notification.type" />
            </li>
        </transition-group>
        <main class="main-wrapper">
            <slot />
        </main>
    </div>
</template>

<script lang="ts">
export default defineComponent({
    data() {
        return {
            showNotification: false,
        }
    },
    computed: {
        activeNotifications() {
            return this.$notifications.value.filter(not => not.active);
        }
    }
})
</script>

<style lang="sass" scoped>
.page-wrapper
    height: 100%

    display: flex
    flex-direction: column

.seperator
    height: 0.2em
    margin-left: 4px
    margin-right: 4px

    background: black
    border-radius: 8px

:deep(.notification-wrapper)
    position: absolute
    width: 16rem
    height: 100vh
    max-height: 100vh
    overflow-y: auto
    pointer-events: none

.notification
    width: 100%
    height: 8rem
    padding: 6px

.notification-list-move, .notification-list-enter-active,.notification-list-leave-active
    transition: all 0.5s ease

.notification-list-enter-from, .notification-list-leave-to
    opacity: 0
    transform: translateX(-100%)

.notification-list-leave-active
    position: absolute

.main-wrapper
    flex-grow: 1

    display: flex
    flex-direction: column
    row-gap: 12px

    justify-content: center
    align-items: center
</style>