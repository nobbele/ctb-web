import { CtbWebApi } from "../withApi";

export const brokenApi: CtbWebApi = {
    type: "broken",

    async register(_data) {
        throw 'Broken Dummy API';
    },

    async login(_data) {
        throw 'Broken Dummy API';
    },

    async getUser(id: number) {
        throw 'Broken Dummy API';
    },

    async findUser(name: string) {
        throw 'Broken Dummy API';
    },

    async getMe() {
        throw 'Broken Dummy API';
    },
}