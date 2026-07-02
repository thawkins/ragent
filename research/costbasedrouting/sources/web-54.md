# Web source

- URL: https://medium.com/medialesson/getting-started-with-model-router-in-azure-ai-foundry-using-c-d17a10681a3f
- Title: [Sitemap][1]
- Captured (UTC): 2026-06-29T15:43:17.357049401+00:00

```text
[Sitemap][1]
[Open in app][2]

Sign up

[Sign in][3]

Get app
[
Write
][4]
[
Search
][5]

Sign up

[Sign in][6]

[Unknown user]
[

## Medialesson

][7]
·
[
[Medialesson]
][8]

We help our customers design, architect, develop and operate modern, intelligent, beautiful and usable apps on any
platform powered by the Cloud, IoT and AI.

# Getting Started with Model Router in Azure AI Foundry Using C#

## Build Smarter AI Apps with Model Router, Azure AI Foundry, and .NET — Route Requests Dynamically with Ease

[
[Sebastian Jensen]
][9]
[Sebastian Jensen][10]
6 min read
·
Jun 19, 2025
[
][11]

--

1

[
][12]
[][13]
[

Listen

][14]

Share

Press enter or click to view image in full size

## Introduction

As AI workloads become increasingly complex, the need of flexibility and scalability in choosing and using different
foundation models is growing. Microsoft addresses this with **Azure AI Foundry**, a platform that simplifies how
developers discover, use, and govern foundation models, including those from OpenAI, Meta, Mistral, and others.

One of the key components in this ecosystem is the **Model Router** — a smart abstraction layer that allows you to
interact with multiple models through a single unified endpoint. Whether you’re building a chatbot, summarizing
documents, or generating code, **Model Router helps route your requests to the best available model**, based on your
defined configuration.

In this post, you’ll learn how to use the Model Router in Azure AI Foundry with a practical example in C# using the
official SDK.

## What Is Azure AI Foundry?

**Azure AI Foundry** is a unified platform within Azure that enables developers to:
* Discover, evaluate, and use multiple foundation models (OpenAI, Llama 2, Mistral, etc.)
* Use familiar APIs with smart routing capabilities
* Deploy and manage models with compliance and observability in mind

It’s designed to **reduce friction in adopting generative AI** by letting developers plug into a single interface while
Azure takes care of the orchestration behind the scenes.

## What Is Model Router?

The **Model Router** in Azure AI Foundry acts as an intelligent middleman. Instead of calling a specific model directly,
you call the **Model Router endpoint**. The router then forwards the request to the appropriate underlying model based
on its configuration. By evaluating factors like query complexity, cost, and performance, it efficiently routes requests
to the most suitable model, ensuring high quality results while minimizing costs. In tests by Microsoft comparing the
use of the Model Router versus the use of GPT-4.1 only, they saw up-to 60% cost savings with similar accuracy.

**Available Models within the Model Router:**
* GPT-4.1, Model version: 2025–04–14
* GPT-4.1-Mini, Model version: 2025–04–14
* GPT-4.1-Nano, Model version: 2025–04–14
* o4-Mini, Model version: 2025–04–16

## Setting Up the Model Router in the Azure Portal

To get started, head over to the [Azure Portal][15] and open your **Azure AI Foundry** resource. If you don’t have one
yet, you can create it by searching for **“Azure AI Foundry”** and following the guided setup.

Once inside your Foundry workspace:
1. Go to the **Deployments** section.
2. Click **“Models + endpoints”** and select **“+ Deploy model”**.
3. Select “**model-router**” from the list of models
4. Define a **name** for the router (e.g., `model-router`).
5. Save the configuration by hitting **confirm **and **deploy**.

💡 **Note:** You don’t need to manually deploy the underlying models yourself. Azure will handle provisioning and
availability behind the scenes when you use the Model Router.

## Sample C# Console App to Use the Model Router

The following is a simple interactive console application in C# that lets you:
* Input your Azure OpenAI endpoint and API key
* Specify the model deployment (router) name
* Chat with the AI in a continuous loop
* See token usage statistics after every response

To improve the styling and user experience in the console application, I’m using a helper class called `ConsoleHelper`.
You can find the full source code in my [GitHub repository][16].

The core logic resides in the `Program.cs` file. At startup, the app prompts the user to enter the necessary
configuration values: the Azure OpenAI endpoint, API key, and the name of the Model Router deployment.

Once configured, the application enters a loop that simulates a chat conversation. For each message, the app sends the
conversation history to the Model Router and displays the AI’s response in real time. At the end of each interaction, it
also prints which model was used and how many tokens were consumed for input and output.

using Azure.AI.OpenAI;
using ModelRouterSample.Utils;
using OpenAI.Chat;
using System.ClientModel;
using ChatMessage = OpenAI.Chat.ChatMessage;

Console.CancelKeyPress += (sender, e) =>
{
    // Prevent the process from terminating immediately
    e.Cancel = true;

    // Inform the user that cancellation was detected (e.g., Ctrl+C)
    ConsoleHelper.WriteToConsole(
        $"{Environment.NewLine}" +
        $"[yellow]Cancellation requested. Exiting...[/]");

    // Exit the application gracefully with a success code (0)
    Environment.Exit(0);
};

// Load configuration from user input
var (endpoint, apiKey, deploymentModel) = GetConfiguration();

// Create Azure OpenAI chat client
var chatClient = new AzureOpenAIClient(
    new Uri(endpoint),
    new ApiKeyCredential(apiKey)).GetChatClient(deploymentModel);

// Display the application header
ConsoleHelper.ShowHeader();

// create a list of chat messages
List<ChatMessage> chatMessages = [];

// chat loop
while (true)
{
    try
    {
        // Prompt the user for input without clearing the console
        string userInput =
            ConsoleHelper.GetString("Enter your message:", false);

        // Add the user message to the ongoing chat history
        chatMessages.Add(ChatMessage.CreateUserMessage(userInput));

        // Print a separator and label for the AI response
        Console.WriteLine();
        Console.WriteLine("AI:");

        // Send the chat history to the Azure OpenAI model for completion
        ClientResult<ChatCompletion> result =
            await chatClient.CompleteChatAsync(chatMessages);

        // Extract the first response from the model output
        string aiResponse =
            result.Value.Content[0].Text;

        // Display the AI's response in the console using Spectre.Console 
        // markup
        ConsoleHelper.WriteToConsole(aiResponse);

        // Add the AI's response to the chat history to maintain context
        chatMessages.Add(ChatMessage.CreateAssistantMessage(aiResponse));

        // Retrieve and display token usage information
        ChatTokenUsage usage =
            result.Value.Usage;
        ConsoleHelper.WriteResponseInformation(
            result.Value.Model,
            usage.InputTokenCount,
            usage.OutputTokenCount);

        // Add extra spacing for the next input loop
        Console.WriteLine();
    }
    catch (Exception ex)
    {
        // Handle and display any unexpected errors
        // (e.g., network or API issues)
        ConsoleHelper.WriteToConsole(
            $"[red]An error occurred: {ex.Message}[/]");
        Console.WriteLine();
    }
}

/// <summary>
/// Prompts the user to enter endpoint, API key, and model name.
/// </summary>
/// <returns>A tuple containing endpoint, API key, and model name.</returns>
static (string Endpoint, string ApiKey, string ModelName) GetConfiguration()
{
    string endpoint =
        ConsoleHelper.GetUrl(
            "Enter your [yellow]Azure OpenAI[/] endpoint:");

    string apiKey =
        ConsoleHelper.GetString(
            "Enter your [yellow]Azure OpenAI[/] API key:");

    string modelName =
        ConsoleHelper.GetString(
            "Enter your [yellow]Model Router[/] model name:");

    return (endpoint, apiKey, modelName);
}

## App in Action

When you launch the application, it will guide you through a short setup process: Enter your Azure OpenAI endpoint.

Press enter or click to view image in full size

Provide your Azure OpenAI API key.

Press enter or click to view image in full size

Specify the deployment name, which points to your configured Model Router.

Press enter or click to view image in full size

Let’s try a simple prompt:

> ***Why is the sky blue?***

The Model Router routes this request to the `gpt-4.1-nano` model, which returns the Das gutexplanation.

Press enter or click to view image in full size

Now let’s try a more complex, open-ended prompt:

> ***Write a comprehensive account of a solo backpacking adventure through South America, covering countries like Chile,
> Brazil, and Argentina. Detail the challenges and triumphs of traveling solo, your interactions with locals, and the
> cultural diversity you encountered along the way. Include must-visit attractions, unique experiences like hiking, and
> recommendations for budget travelers.***

In this case, the Model Router intelligently selects a more capable model — such as `o4-mini`—to handle the increased
complexity and generate a detailed, narrative-style response.

As you can see, the Model Router dynamically routes requests to the most suitable model based on your input — **no code
changes needed.**

Press enter or click to view image in full size

## Wrapping Up

Azure AI Foundry’s Model Router enables developers to **decouple application logic from specific models**, offering a
powerful way to scale and adapt your AI workflows. Combined with the official C# SDK, you can get up and running
quickly, securely, and with production-level reliability.

You can explore the complete source code, including `Program.cs` and the `ConsoleHelper` class, in my [GitHub
repository][17].

[
AI
][18]
[
Azureaifoundry
][19]
[
Azure
][20]
[
Dotnet
][21]
[
ChatGPT
][22]
[
][23]

--

[
][24]

--

1

[
][25]
[][26]
[
[Medialesson]
][27]
[
[Medialesson]
][28]
[

## Published in Medialesson

][29]
[461 followers][30]
·[Last published 5 days ago][31]

We help our customers design, architect, develop and operate modern, intelligent, beautiful and usable apps on any
platform powered by the Cloud, IoT and AI.

[
[Sebastian Jensen]
][32]
[
[Sebastian Jensen]
][33]
[

## Written by Sebastian Jensen

][34]
[351 followers][35]
·[38 following][36]

Senior Software Developer & Team Lead @ Medialesson GmbH

[

Help

][37]
[

Status

][38]
[

About

][39]
[

Careers

][40]
[

Press

][41]
[

Blog

][42]
[

Store

][43]
[

Privacy

][44]
[

Rules

][45]
[

Terms

][46]
[

Text to speech

][47]

[1]: /sitemap/sitemap.xml
[2]: https://play.google.com/store/apps/details?id=com.medium.reader&referrer=utm_source%3DmobileNavBar&source=post_page
---top_nav_layout_nav-----------------------------------------
[3]: /m/signin?operation=login&redirect=https%3A%2F%2Fmedium.com%2Fmedialesson%2Fgetting-started-with-model-router-in-az
ure-ai-foundry-using-c-d17a10681a3f&source=post_page---top_nav_layout_nav-----------------------global_nav--------------
----
[4]: /m/signin?operation=register&redirect=https%3A%2F%2Fmedium.com%2Fnew-story&source=---top_nav_layout_nav------------
-----------new_post_topnav------------------
[5]: /search?source=post_page---top_nav_layout_nav-----------------------------------------
[6]: /m/signin?operation=login&redirect=https%3A%2F%2Fmedium.com%2Fmedialesson%2Fgetting-started-with-model-router-in-az
ure-ai-foundry-using-c-d17a10681a3f&source=post_page---top_nav_layout_nav-----------------------global_nav--------------
----
[7]: https://medium.com/medialesson?source=post_page---publication_nav-2e91bf3dad1c-d17a10681a3f------------------------
---------------
[8]: https://medium.com/medialesson?source=post_page---post_publication_sidebar-2e91bf3dad1c-d17a10681a3f---------------
------------------------
[9]: /@tsjdevapps?source=post_page---byline--d17a10681a3f---------------------------------------
[10]: /@tsjdevapps?source=post_page---byline--d17a10681a3f---------------------------------------
[11]: /m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Fvote%2Fmedialesson%2Fd17a10681a3f&operation=register&redirect=h
ttps%3A%2F%2Fmedium.com%2Fmedialesson%2Fgetting-started-with-model-router-in-azure-ai-foundry-using-c-d17a10681a3f&user=
Sebastian+Jensen&userId=c8f6762e0e4b&source=---header_actions--d17a10681a3f---------------------clap_footer-------------
-----
[12]: /m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Frepost%2Fp%2Fd17a10681a3f&operation=register&redirect=https%3A%
2F%2Fmedium.com%2Fmedialesson%2Fgetting-started-with-model-router-in-azure-ai-foundry-using-c-d17a10681a3f&user=Sebastia
n+Jensen&userId=c8f6762e0e4b&source=---header_actions--d17a10681a3f---------------------repost_header------------------
[13]: /m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Fbookmark%2Fp%2Fd17a10681a3f&operation=register&redirect=https%3
A%2F%2Fmedium.com%2Fmedialesson%2Fgetting-started-with-model-router-in-azure-ai-foundry-using-c-d17a10681a3f&source=---h
eader_actions--d17a10681a3f---------------------bookmark_footer------------------
[14]: /m/signin?actionUrl=https%3A%2F%2Fmedium.com%2Fplans%3Fdimension%3Dpost_audio_button%26postId%3Dd17a10681a3f&opera
tion=register&redirect=https%3A%2F%2Fmedium.com%2Fmedialesson%2Fgetting-started-with-model-router-in-azure-ai-foundry-us
ing-c-d17a10681a3f&source=---header_actions--d17a10681a3f---------------------post_audio_button------------------
[15]: https://portal.azure.com/
[16]: https://github.com/tsjdev-apps/ai-model-router
[17]: https://github.com/tsjdev-apps/ai-model-router
[18]: /tag/ai?source=post_page-----d17a10681a3f---------------------------------------
[19]: /tag/azureaifoundry?source=post_page-----d17a10681a3f---------------------------------------
[20]: /tag/azure?source=post_page-----d17a10681a3f---------------------------------------
[21]: /tag/dotnet?source=post_page-----d17a10681a3f---------------------------------------
[22]: /tag/chatgpt?source=post_page-----d17a10681a3f---------------------------------------
[23]: /m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Fvote%2Fmedialesson%2Fd17a10681a3f&operation=register&redirect=h
ttps%3A%2F%2Fmedium.com%2Fmedialesson%2Fgetting-started-with-model-router-in-azure-ai-foundry-using-c-d17a10681a3f&user=
Sebastian+Jensen&userId=c8f6762e0e4b&source=---footer_actions--d17a10681a3f---------------------clap_footer-------------
-----
[24]: /m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Fvote%2Fmedialesson%2Fd17a10681a3f&operation=register&redirect=h
ttps%3A%2F%2Fmedium.com%2Fmedialesson%2Fgetting-started-with-model-router-in-azure-ai-foundry-using-c-d17a10681a3f&user=
Sebastian+Jensen&userId=c8f6762e0e4b&source=---footer_actions--d17a10681a3f---------------------clap_footer-------------
-----
[25]: /m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Frepost%2Fp%2Fd17a10681a3f&operation=register&redirect=https%3A%
2F%2Fmedium.com%2Fmedialesson%2Fgetting-started-with-model-router-in-azure-ai-foundry-using-c-d17a10681a3f&user=Sebastia
n+Jensen&userId=c8f6762e0e4b&source=---footer_actions--d17a10681a3f---------------------repost_footer------------------
[26]: /m/signin?actionUrl=https%3A%2F%2Fmedium.com%2F_%2Fbookmark%2Fp%2Fd17a10681a3f&operation=register&redirect=https%3
A%2F%2Fmedium.com%2Fmedialesson%2Fgetting-started-with-model-router-in-azure-ai-foundry-using-c-d17a10681a3f&source=---f
ooter_actions--d17a10681a3f---------------------bookmark_footer------------------
[27]: https://medium.com/medialesson?source=post_page---post_publication_info--d17a10681a3f-----------------------------
----------
[28]: https://medium.com/medialesson?source=post_page---post_publication_info--d17a10681a3f-----------------------------
----------
[29]: https://medium.com/medialesson?source=post_page---post_publication_info--d17a10681a3f-----------------------------
----------
[30]: /medialesson/followers?source=post_page---post_publication_info--d17a10681a3f-------------------------------------
--
[31]: /medialesson/implementing-barcode-scanning-in-microsoft-teams-apps-7a26d2f04d6a?source=post_page---post_publicatio
n_info--d17a10681a3f---------------------------------------
[32]: /@tsjdevapps?source=post_page---post_author_info--d17a10681a3f---------------------------------------
[33]: /@tsjdevapps?source=post_page---post_author_info--d17a10681a3f---------------------------------------
[34]: /@tsjdevapps?source=post_page---post_author_info--d17a10681a3f---------------------------------------
[35]: /@tsjdevapps/followers?source=post_page---post_author_info--d17a10681a3f---------------------------------------
[36]: /@tsjdevapps/following?source=post_page---post_author_info--d17a10681a3f---------------------------------------
[37]: https://help.medium.com/hc/en-us?source=post_page-----d17a10681a3f---------------------------------------
[38]: https://status.medium.com/?source=post_page-----d17a10681a3f---------------------------------------
[39]: /about?autoplay=1&source=post_page-----d17a10681a3f---------------------------------------
[40]: /jobs-at-medium/work-at-medium-959d1a85284e?source=post_page-----d17a10681a3f-------------------------------------
--
[41]: mailto:pressinquiries@medium.com
[42]: https://blog.medium.com/?source=post_page-----d17a10681a3f---------------------------------------
[43]: https://medium.com/store
[44]: https://policy.medium.com/medium-privacy-policy-f03bf92035c9?source=post_page-----d17a10681a3f--------------------
-------------------
[45]: https://policy.medium.com/medium-rules-30e5502c4eb4?source=post_page-----d17a10681a3f-----------------------------
----------
[46]: https://policy.medium.com/medium-terms-of-service-9db0094a1e0f?source=post_page-----d17a10681a3f------------------
---------------------
[47]: https://speechify.com/medium?source=post_page-----d17a10681a3f---------------------------------------
```
