<template>
    <form @submit.prevent="submit">
        <div class="entries">
            <label for="form-username">Username:</label>
            <input type="text" id="form-username" placeholder="john_doe1337" required v-model="username">

            <label for="form-email">Email:</label>
            <input type="email" id="form-email" placeholder="john.doe@provider.com" required v-model="email">

            <label for="form-password">Password:</label>
            <input type="password" id="form-password" placeholder="&#9679;&#9679;&#9679;&#9679;&#9679;&#9679;&#9679;"
                required v-model="password">
        </div>
        <input type="submit" value="Submit">
    </form>
</template>

<script lang="ts">
export interface RegistrationData {
    username: String,
    email: String,
    password: String
}

interface Data {
    username: String | null,
    email: String | null,
    password: String | null
}

export default defineComponent({
    emits: ["handle"],
    data(): Data {
        return {
            username: null,
            email: null,
            password: null
        }
    },
    methods: {
        submit() {
            // Username, email and password are validated to not be null by <form>
            const data: RegistrationData = {
                username: this.username!,
                email: this.email!,
                password: this.password!,
            };
            this.$emit("handle", data);
        }
    }
});
</script>

<style lang="sass" scoped>
form
    display: flex
    flex-direction: column

    align-items: center

.entries
    display: inline-grid
    grid-template-columns: auto auto

    align-items: center

input
    background-color: white
    border-radius: 4px

    margin: 8px
    padding: 2px

input[type=submit]
    width: fit-content
    cursor: pointer

    transition: all .15s ease-in-out

    &:hover, &:active
        transform: scale(1.1)
        box-shadow: 0 3px 3px 0

</style>