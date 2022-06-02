export interface RegistrationData {
    username: String,
    email: String,
    password: String
}

interface CtbWebApi {
    register: (data: RegistrationData) => Promise<void>,
}

type ApiWithBase = (baseUrl: string) => CtbWebApi;
const realApi: ApiWithBase = baseUrl => ({
    async register(data) {
        const result = await $fetch<string>(`${baseUrl}/register`, {
            method: 'POST',
            body: data
        });

        if (result != "Success") {
            throw 'Not success Error';
        }
    }
});

const dummyApi: CtbWebApi = {
    async register(data) {
        console.log("Dummy registration..");
        console.log(data);
    }
}

const brokenApi: CtbWebApi = {
    async register(data) {
        throw 'Broken Dummy API';
    }
}

type ApiType = 'dummy' | 'real' | 'broken';
function getApi(type: ApiType): CtbWebApi {
    switch (type) {
        case 'dummy':
            return dummyApi;
        case 'real':
            return realApi("http://127.0.0.1:8080");
        case 'broken':
            return brokenApi;
    }
}

export default defineNuxtPlugin(() => {
    return {
        provide: {
            ctbWebApi: getApi('dummy'),
        }
    }
});