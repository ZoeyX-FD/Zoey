hey im Zoey 

this is My eXperimental Project on AI , purpose for learning 

im newbie , just only start 1st learning coding in early january 2025

im use Arc Rig Framework on rust , my 1st programming languange 

for agent crypto folder mostly based idea by moondev ( coingecko agent , newtop agent ) but modified and work in rust

u can start by this for crypto research , 

disclaimer this is not trading Agent, but Research crypto agent or coin gecko agent 😁

u must install Rust before begin 

https://www.rust-lang.org/tools/install

clone my repo and start

===================

cargo run -p crypto-agents 

===================

before that u must have coingecko api 
( im using free api coingecko ) 

must have twitter account , add cookie too , u can check .env.example and then copy changes to .env 

to get cookie , you can check on folder agent-twitter-client 

 im use diffrent provider ( deepseek, mistral , openai, openrouter ,gemini or cohere ) if uhave just 1 provider , is ok , provider like cohere , gemini , mistral are free 

add your provider api key on .env too 

and then add too by terminal 

export  ( YOURPROVIDER_API_KEY )

u can setting in .env too for this configuration 

====================

u can check my example 

cargo run --example coin_analysis 

coin analysis agent for see coin you want watch and then linked to twitter search for sentiment social twitter for that coin 

make sure the coin and name coin same like the website coingecko 

SOl , solana

JUP , jupiter

BTC , bitcoin

====================

cargo run --example topic_insight

this topic insight agent can search ur choosing topic and then search to twitter for see sentiment analysis 

example topic = agent ai , solana ecosystem , or bitcoin , or u can choose freely, the agent can search based your choose topic and give sentiment result 

u can cheks more in my example in folder crypto-agents 

 - teknikal analysis agent 

cargo run --example technical_analysis -- (namecoin) (nameyourprovider) (nameyourprovider - model )

cargo run --example technical_analysis -- bitcoin deepseek deepseek-chat

- scraping twitter user 

cargo run --example twitter_user_extract (username) (number of tweet) (number of day) ai

cargo run --example twitter_user_extract aixbt_agent 50 1 ai

====================

cargo run -p zoey-rag

this is for general , purpose for research and direct chat 

u must have cohere api key or another provider , because for embedding im using this , and for deafult im using mistral provider 

he can read and ingest you document or website 

/load ( nameyourdocument)

ur document pdf or txt  must put in under documents folder 

he can scrape and search the website use exa search and u can chat too about that (must have exa API key)


========================

to check provider like mistral and openrouter work , u can check this , because this custom module in common folder , not in rig core , but integrated to rig framework 

cargo run --example mistral_trading

cargo run --example openrouter_example

========================

add twitter bot 

use config twitter_config.json for config twitter bot 

cargo run -p zoey

========================



special Credits and thanks for

=======================================

@moondev for my inspiration and my idol , i learn many things from you 
@Arcdotfun  Rig-framework 

========================================
@agent-twitter-clients , Core main , twitter logic , Trader-Solana , with modified 
original project - created by = Rina ( https://github.com/cornip/Rina)


========================================

changelog 
update 19 feb 2025 - add twitter bot functionality and trading solana - by RINA  and integrated with RIG 0.8.0 , fix some bug and add more example 
