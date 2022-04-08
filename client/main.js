import init, { 
    initialize,
    generate_public_parameters,
    generate_query,
    decode_response
} from './pkg/client.js';

import './js/bz2.js';
import './js/wtf_wikipedia.js';
import './js/wtf-plugin-html.js';
wtf.extend(wtfHtml);

const API_URL = "https://spiralwiki.com:8088";
const SETUP_URL = "/setup";
const QUERY_URL = "/query";

async function postData(url = '', data = {}, json = false) {
    const response = await fetch(url, {
      method: 'POST',
      mode: 'cors',
      cache: 'no-cache',
      credentials: 'omit',
      headers: { 'Content-Type': 'application/octet-stream' },
      redirect: 'follow',
      referrerPolicy: 'no-referrer',
      body: data
    });
    if (json) {
        return response.json();
    } else {
        let data = await response.arrayBuffer();
        return new Uint8Array(data);
    }
}

async function getData(url = '', json = false) {
    const response = await fetch(url, {
      method: 'GET',
      cache: 'no-cache',
      credentials: 'omit',
      redirect: 'follow',
      referrerPolicy: 'no-referrer'
    });
    if (json) {
        return response.json();
    } else {
        let data = await response.arrayBuffer();
        return new Uint8Array(data);
    }
}

const api = {
    setup: async (data) => postData(API_URL + SETUP_URL, data, true),
    query: async (data) => postData(API_URL + QUERY_URL, data, false)
}

function preprocessWikiText(wikiText, targetTitle) {
    targetTitle = targetTitle.toLowerCase();

    wikiText = wikiText
        // .replace(/<title>(.*?)<\/title><text>/gi, "<text>\n\n<h1>$1</h1>\n\n")
        .replace(/&lt;ref&gt;[\s\S]*?&lt;\/ref&gt;/gi, "")
        .replace(/&lt;ref[\s\S]*?&lt;\/ref&gt;/gi, "")
        .replace(/&lt;ref[\s\S]*?\/&gt;/gi, "")
        .replace(/&lt;![\s\S]*?--&gt;/gi, "");
    
    let articles = wikiText.split("<title>")
        .filter(d => d.length > 10)
        .filter(d => {
            var title = "";
            var endTitleTagIdx = d.indexOf("</title>");
            if (endTitleTagIdx != -1) {
                title = d.slice(0, endTitleTagIdx);
            }
            return title.toLowerCase() == targetTitle;
        });

    if (articles.length === 0) {
        console.log("error decoding...");
        return "";
    }

    let d = articles[0];
    let articlePageMatch = d.match(/<text>/);
    if (!articlePageMatch) {
        console.log("error decoding...");
        return "";
    }
    let startPageContentIdx = articlePageMatch.index + articlePageMatch[0].length;
    let endPageContentIdx = d.slice(startPageContentIdx).indexOf("</text>")
    d = d.slice(startPageContentIdx, endPageContentIdx);
    return d;
}
function postProcessWikiHTML(wikiHTML, title) {
    wikiHTML = wikiHTML.replace(/<img.*?\/>/g, "");
    wikiHTML = "<h2 class=\"title\">"+title+"</h2>" + wikiHTML
    return wikiHTML;
}

function resultToHtml(result, title) {
    let decompressedData = bz2.decompress(result);
    let wikiText = new TextDecoder("utf-8").decode(decompressedData);
    wikiText = preprocessWikiText(wikiText, title);
    console.log(wikiText);
    let wikiHTML = wtf(wikiText).html();
    wikiHTML = postProcessWikiHTML(wikiHTML, title);
    return "<article>" + wikiHTML + "</article>";    
}
window.resultToHtml = resultToHtml;

function addBold(suggestion, query) {
    return '<span class="highlight">'
        + suggestion.slice(0,query.length) 
        + "</span>"
        + suggestion.slice(query.length);
}

function showSuggestionsBox(suggestions, query) {
    var htmlSuggestions = '<div class="suggestions">' 
        + suggestions.map(m => "<div>"+addBold(m, query)+"</div>").join('') 
        + "</div>";
    document.querySelector('.searchbox').insertAdjacentHTML('afterend', htmlSuggestions);
    document.querySelectorAll('.suggestions > div').forEach((el) => {
        el.onclick = (e) => {
            document.querySelector(".searchbox").value = el.innerHTML
                .replace('<span class="highlight">', '')
                .replace('</span>', '');
            clearExistingSuggestionsBox();
            document.querySelector('#make_query').click();
        }
    });
}

function clearExistingSuggestionsBox() {
    var existing = document.querySelector('.suggestions');
    if (existing) {
        existing.remove();
    }
}

function hasTitle(title) {
    return window.title_index.hasOwnProperty(title) && window.title_index[title] < window.numArticles;
}

function followRedirects(title) {
    if (hasTitle(title)) {
        return title;
    } else if (window.redirects.hasOwnProperty(title) && hasTitle(window.redirects[title])) {
        return window.redirects[title];
    } else {
        return null;
    }
}

function queryTitleOnClick(title) {
    return async () => {
        queryTitle(title);
        return false;
    }
}

function enableLinks(element) {
    element.querySelectorAll('a').forEach((el) => {
        var linkTitle = el.getAttribute("href").slice(2).replace(/_/g, " ").toLowerCase();
        if (hasTitle(linkTitle)) {
            el.onclick = queryTitleOnClick(linkTitle);
        } else {
            var redirected = followRedirects(linkTitle);
            if (redirected !== null && hasTitle(redirected)) {
                el.onclick = queryTitleOnClick(redirected);
            } else {
                el.classList.add("broken")
            }
        }
    })
}

async function query(targetIdx, title) {    
    if (!window.hasSetUp) {
        console.log("Initializing...");
        window.client = initialize();
        console.log("done");
        console.log("Generating public parameters...");
        let publicParameters = generate_public_parameters(window.client);
        console.log(`done (${publicParameters.length} bytes)`);
        console.log("Sending public parameters...");
        let setup_resp = await api.setup(publicParameters);
        console.log("sent.");
        console.log(setup_resp);
        window.id = setup_resp["id"];
        window.hasSetUp = true;
    }

    console.log("Generating query... ("+targetIdx+")");
    let query = generate_query(window.client, window.id, targetIdx);
    console.log(`done (${query.length} bytes)`);

    console.log("Sending query...");
    let response = await api.query(query);
    console.log("sent.");

    console.log(`done, got (${response.length} bytes)`);
    console.log(response);

    console.log("Decoding result...");
    let result = decode_response(window.client, response)
    console.log("done.")
    console.log("Final result:")
    console.log(result);

    let resultHtml = resultToHtml(result, title);

    let outputArea = document.getElementById("output");
    outputArea.innerHTML = resultHtml;

    enableLinks(outputArea);
}

async function queryTitle(targetTitle) {
    let redirectedTitle = followRedirects(targetTitle);
    let articleIndex = window.title_index[redirectedTitle];
    return await query(articleIndex, targetTitle);
}

async function run() {
    await init();

    window.numArticles = 65536;
    window.articleSize = 100000;

    let makeQueryBtn = document.querySelector('#make_query');
    let searchBox = document.querySelector(".searchbox");

    window.sample_data = await getData("sample.dat");
    window.title_index = await getData("enwiki-20220320-index.json", true);
    let keys = Object.keys(window.title_index);
    for (var i = 0; i < keys.length; i++) {
        let key = keys[i];
        window.title_index[key] /= window.articleSize; 
        window.title_index[key.toLowerCase()] = window.title_index[key];
    }
    let redirect_backlinks = await getData("redirects-old.json", true);
    keys = Object.keys(redirect_backlinks);
    window.redirects = {}
    for (var i = 0; i < keys.length; i++) {
        let redirect_dest = keys[i];
        let redirect_srcs = redirect_backlinks[redirect_dest];
        for (var j = 0; j < redirect_srcs.length; j++) {
            window.redirects[redirect_srcs[j].toLowerCase()] = redirect_dest;
        }
    }

    searchBox.addEventListener('input', (e) => {
        clearExistingSuggestionsBox();
    
        let search = e.target.value;
        if (search.length < 1) return;
    
        var matching = Object.keys(window.title_index).filter((v) => v.startsWith(search));
        if (matching.length == 0) return;
    
        matching.sort();
        if (matching.length > 10) matching = matching.slice(0, 10);
    
        showSuggestionsBox(matching, search);
    })

    makeQueryBtn.onclick = async () => {
        makeQueryBtn.disabled = true;
        await queryTitle(searchBox.value);
        makeQueryBtn.disabled = false;
    }
}
run();