import { brokenApi } from "./apis/broken";
import { dummyApi } from "./apis/dummy";
import { realApi } from "./apis/real";

export interface RegistrationData {
    username: String,
    email: String,
    password: String
}

export interface LoginData {
    username: String,
    password: String
}

export interface UserData {
    id: number,
    username: string,
    email: string | null,
}

export interface CtbWebApi {
    type: string,
    register: (data: RegistrationData) => Promise<void>,
    login: (data: LoginData) => Promise<string>,
    getUser: (id: number) => Promise<UserData | null>,
    findUser: (name: string) => Promise<UserData | null>,
    getMe: () => Promise<UserData | null>,
}

export type ApiType = 'dummy' | 'real' | 'broken';
export const currentApiType: ApiType = 'real';
export function withApi(): CtbWebApi {
    const config = useRuntimeConfig();
    switch (currentApiType) {
        case 'dummy':
            return dummyApi;
        case 'real':
            return realApi(config.public.apiBase);
        case 'broken':
            return brokenApi;
    }
}