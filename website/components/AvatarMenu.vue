<template>
    <a href="#">Profile</a>
    <a href="#">Friends</a>
    <a href="#">Settings</a>
    <a href="#" @click="handleLogout">Logout</a>
</template>

<script lang="ts">
export default defineComponent({
    methods: {
        async handleLogout() {
            await this.$unsyncToken();
            this.$notify("Logout Successful.");
        }
    },
});
</script>

<style lang="sass" scoped>

</style>