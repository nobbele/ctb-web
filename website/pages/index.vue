<template>
    <div>
        <p>Welcome to CTB Web!</p>
        <button @click="$notify('This is a test notification')">Notify</button>
        <br />
        <button @click="$notify('This is a test notification', 'failure')">Notify Failure</button>
    </div>
</template>

<script lang="ts">
export default defineComponent({

});
</script>

<style lang="sass" scoped>

</style>