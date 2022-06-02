<template>
    <div class="game-wrapper">
        <iframe class="game-frame" src="http://127.0.0.1:4000" allow="fullscreen" />
    </div>
</template>

<script lang="ts">
export default defineComponent({

});
</script>

<style lang="sass" scoped>
.game-wrapper
    display: flex
    width: 100%
    flex: 1

    justify-content: center
    align-items: center

.game-frame
    flex: 1
    height: 100%
</style>