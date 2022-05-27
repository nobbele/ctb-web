<template>
    <div class="pageWrapper">
        <h1>Register</h1>
        <div class="formWrapper">
            <registration-form @handle="handleRegistration" />
        </div>
    </div>
</template>

<script lang="ts">
import { RegistrationData } from "components/RegistrationForm.vue";

export default defineComponent({
    methods: {
        async handleRegistration(data: RegistrationData) {
            const result = await $fetch("http://127.0.0.1:8080/register", {
                method: 'POST',
                body: data
            });

            if (result != "Success") {
                // TODO Alert
            }
        },
    },
});
</script>

<style lang="sass" scoped>
h1
    font-size: 24pt

.pageWrapper
    display: flex
    height: 100%

    flex-direction: column
    row-gap: 12px

    justify-content: center
    align-items: center

.formWrapper
    background-color: #FFFFFF60
    box-shadow: 0 8px 8px 0
    padding: 8px
</style>