import { useToken } from '~~/composables/useToken';
import { CtbWebApi, UserData } from "../withApi";

type ApiWithBase = (baseUrl: string) => CtbWebApi;
export const realApi: ApiWithBase = baseUrl => ({
    type: "dev",

    async register(data) {
        const result = await $fetch<string>(`${baseUrl}/register`, {
            method: 'POST',
            body: data
        });

        if (result == "invalid-data") {
            throw 'Invalid Data';
        }
    },

    async login(data) {
        const result = await $fetch<any>(`${baseUrl}/login`, {
            method: 'POST',
            body: data
        });

        if (result == "invalid-credentials") {
            throw 'Invalid Credentials Error';
        }

        if (result.token == undefined) {
            throw 'Invalid Response';
        }

        return result.token;
    },

    async getUser(id: number) {
        throw 'unimplemented getUser';
    },

    async findUser(name: string) {
        throw 'unimplemented findUser';
    },

    async getMe() {
        const token = useToken();
        return await $fetch<UserData>(`${baseUrl}/users/me`, {
            method: 'GET',
            headers: { Authorization: `Bearer ${token.value}` }
        });
    },
});