<template>
    <div class="wrapper" :class="{ success: type == 'success', failure: type == 'failure' }">
        {{ message }}
    </div>
</template>

<script lang="ts">
export default defineComponent({
    props: {
        message: {
            type: String,
            required: true,
        },
        type: {
            type: String,
            required: true,
            validator(value) {
                return value == "success" || value == "failure"
            }
        }
    }
});
</script>

<style lang="sass" scoped>
@use '@/assets/general.sass'

.success
    background: #35FF69
    color: black

.failure
    background: #FF3C38
    color: white

.wrapper
    height: 100%
    display: flex

    justify-content: center
    align-items: center

    border-radius: 4px
    box-shadow: 0 3px 3px 0 black
</style>