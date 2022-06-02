<template>
    <header class="header-wrapper">
        <div />
        <nav class="navigation">
            <NuxtLink to="/">Home</NuxtLink>
            <NuxtLink to="/play">Play</NuxtLink>
            <NuxtLink to="/registration">Register</NuxtLink>
        </nav>
        <div class="profile-button" @click="showProfileMenu = !showProfileMenu;">
            <img src="/assets/Guest.png" role="button" height="48" />
            <div class="profile-popup">
                <div class="profile-menu" :class="{ 'profile-menu-visible': showProfileMenu }">
                    <p> -- Profile -- </p>
                    <a>Button A</a>
                    <a>Button B</a>
                </div>
            </div>
        </div>
    </header>
</template>

<script lang="ts">
export default defineComponent({
    data() {
        return {
            showProfileMenu: false
        }
    }
});
</script>

<style lang="sass" scoped>
@use '@/assets/general.sass'

.navigation
    display: flex

    justify-content: center
    align-items: center

.navigation > a
    +general.plain-button

.header-wrapper
    display: grid
    grid-template-columns: repeat(3, 1fr)

    padding: 6px

.profile-button
    cursor: pointer
    margin-left: auto
    margin-right: 45px
    border-radius: 50%

    display: flex
    position: relative

.profile-button > img
    display: block
    border-radius: 50%

.profile-popup
    position: absolute
    top: 100%
    right: -45px
    height: 0
    margin-top: 4px
    visibility: hidden

.profile-menu-visible
    visibility: visible

.profile-menu
    background: green
    display: flex
    flex-direction: column
    align-items: center

    border-radius: 8px
    padding: 8px

    width: 10em

    box-shadow: 0 2px 6px 0

</style>