<template>
    <div>
        <h1>Register</h1>
        <div class="form-wrapper">
            <registration-form @handle="handleRegistration" />
        </div>
    </div>
</template>

<script lang="ts">
import { RegistrationData, withApi } from "~~/composables/withApi";

export default defineComponent({
    methods: {
        async handleRegistration(data: RegistrationData) {
            try {
                await withApi().register(data);
                this.$notify("Registration Successful.");
            } catch (reason) {
                this.$notify(`Registration Failed.. Reason: ${reason}`, 'failure');
            }
        },
    },
});
</script>

<style lang="sass" scoped>
h1
    font-size: 24pt

.form-wrapper
    background-color: #FFFFFF60
    box-shadow: 0 8px 8px 0
    padding: 8px
</style>