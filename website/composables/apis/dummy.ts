import { useToken } from "../useToken";
import { CtbWebApi } from "../withApi";

export const dummyApi: CtbWebApi = {
    type: "dummy",

    async register(data) {
        console.log("Dummy registration..");
        console.log(data);
    },

    async login(data) {
        if (data.username == "GamerDuck" && data.password == "password123") {
            return "dummy";
        } else {
            throw "Invalid Credentials";
        }
    },

    async getUser(id: number) {
        if (id == 1) {
            return {
                id,
                username: "GamerDuck",
                email: "GamerDuck123@email.com"
            }
        } else {
            return null;
        }
    },

    async findUser(name: string) {
        throw 'unimplemented';
    },

    async getMe() {
        const token = useToken();
        if (token.value == "dummy") {
            return this.getUser(1);
        } else {
            return null;
        }
    },
}