use yew::prelude::*;

#[function_component(Login)]
fn login() -> Html {
    let onsubmit = {
        Callback::from(|e: FocusEvent| {
            e.prevent_default();
        })
    };

    html! {
        <div class="h-full bg-indigo-600">
            <div class="flex justify-center items-center flex-col w-6/12 mx-auto my-0 p-6">
                <h1 class="text-4xl">{"Welcome to CTB Web"}</h1>
                <form class="flex flex-col gap-1" method="post" {onsubmit}>
                    <label for="name">{"Username:"}</label>
                    <input type="text" id="name" name="name" required=true />

                    <label for="pw">{"Password:"}</label>
                    <input type="password" id="pw" name="pw" required=true />

                    <br />

                    <input type="submit" value="Login" class="bg-blue-500 hover:bg-blue-700 text-white font-bold py-2 px-4 border border-blue-700 rounded" />
                </form>
            </div>
        </div>
    }
}

fn main() {
    println!("Starting website..");
    yew::start_app::<Login>();
}
