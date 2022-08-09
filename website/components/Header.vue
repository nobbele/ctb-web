<template>
    <header class="header-wrapper">
        <div />
        <nav class="navigation">
            <NuxtLink to="/">Home</NuxtLink>
            <NuxtLink to="/play">Play</NuxtLink>
            <NuxtLink to="/registration" v-if="!logged_in">Register</NuxtLink>
        </nav>
        <div class="profile">
            <p class="profile-name">{{ $userdata.value?.username || "Guest" }}</p>
            <img class="profile-button" src="/assets/Guest.png" role="button" height="48"
                @click="handleOpenProfileMenu" />
            <div class="profile-popup">
                <div class="profile-menu" :class="{ 'profile-menu-visible': showProfileMenu, 'login-menu': !logged_in }"
                    v-click-outside="handleOutsideClick">
                    <template v-if="logged_in">
                        <avatar-menu />
                    </template>
                    <template v-else>
                        <login-form @handle="handleLogin" />
                    </template>
                </div>
            </div>
        </div>
    </header>
</template>

<script lang="ts">
import { LoginData, withApi } from "~~/composables/withApi";;

export default defineComponent({
    data() {
        return {
            showProfileMenu: false,
        }
    },

    computed: {
        logged_in() {
            return this.$userdata.value != null;
        }
    },

    methods: {
        async handleLogin(data: LoginData) {
            try {
                const token = await withApi().login(data);
                await this.$syncToken(token);
                this.$notify("Login Successful.");
            } catch (reason) {
                this.$notify(`Login Failed.. Reason: ${reason}`, 'failure');
            }
        },

        async handleLogout() {
            await this.$unsyncToken();
            this.$notify("Logout Successful.");
        },

        handleOpenProfileMenu() {
            setTimeout(() => this.showProfileMenu = true, 10);
        },

        handleOutsideClick() {
            this.showProfileMenu = false;
        },
    },
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

.profile-name
    margin-right: 6px

.profile
    margin-left: auto
    margin-right: 58px
    border-radius: 50%

    display: flex
    position: relative
    align-items: center

.profile-button
    display: block
    border-radius: 50%
    cursor: pointer

.profile-popup
    position: absolute
    top: 100%
    right: calc((48px - 10rem) / 2)
    height: 0
    margin-top: 4px

.profile-menu.profile-menu-visible
    opacity: 100%

.profile-menu
    background: green
    display: flex
    flex-direction: column
    align-items: center

    border-radius: 8px
    padding: 8px

    width: 10rem

    box-shadow: 0 2px 6px 0

    opacity: 0

    transition: all .10s ease-in-out

.profile-menu.login-menu
    width: 18em

</style>