import init, { 
    initialize,
    generate_public_parameters,
    generate_query,
    decode_response
} from '../pkg/client.js';

import './bz2.js';
import './wtf_wikipedia.js';
import './wtf-plugin-html.js';
wtf.extend(wtfHtml);

const API_URL = "/api";
const CHECK_URL = "/check";
const SETUP_URL = "/setup";
const QUERY_URL = "/query";

async function postData(url = '', data = {}, json = false) {
    // const response = await fetch(url, {
    //   method: 'POST',
    //   mode: 'cors',
    //   cache: 'no-store',
    //   credentials: 'omit',
    //   headers: { 
    //       'Content-Type': 'application/octet-stream',
    //       'Content-Length': data.length
    //   },
    //   redirect: 'follow',
    //   referrerPolicy: 'no-referrer',
    //   body: data
    // });
    // if (json) {
    //     return response.json();
    // } else {
    //     let data = await response.arrayBuffer();
    //     return new Uint8Array(data);
    // }

    // Can't use Fetch API here since it lacks progress indication
    const xhr = new XMLHttpRequest();
    xhr.responseType = json ? 'json' : 'arraybuffer';
    return await new Promise((resolve, reject) => {
        xhr.upload.addEventListener("progress", (event) => {
            if (event.lengthComputable) {
                setProgress(Math.round(event.loaded / event.total * 100))
            }
        });
        xhr.addEventListener("loadend", () => {
            resolve(xhr.readyState === 4 && xhr.status === 200);
        });
        xhr.onload = function () {
            if (xhr.status >= 200 && xhr.status < 300) {
                resolve(xhr.response);
            } else {
                reject({
                    status: xhr.status,
                    statusText: xhr.statusText
                });
            }
        };
        xhr.onerror = function () {
            reject({
                status: xhr.status,
                statusText: xhr.statusText
            });
        };
        xhr.open("POST", url, true);
        xhr.setRequestHeader("Content-Type", "application/octet-stream");
        xhr.send(new Blob([data.buffer]));
    });
}

async function getData(url = '', json = false) {
    const response = await fetch(url, {
      method: 'GET',
      cache: 'default',
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
    check: async (uuid) => getData(API_URL + CHECK_URL + "?uuid="+uuid, true),
    setup: async (data) => postData(API_URL + SETUP_URL, data, true),
    query: async (data) => postData(API_URL + QUERY_URL, data, false)
}

function extractTitle(article) {
    var title = "";
    var endTitleTagIdx = article.indexOf("</title>");
    if (endTitleTagIdx != -1) {
        title = article.slice(0, endTitleTagIdx);
    }
    return title;
}

function preprocessWikiText(wikiText, targetTitle) {
    targetTitle = targetTitle.toLowerCase();
    
    let articles = wikiText.split("<title>")
        .filter(d => d.length > 10)
        .filter(d => {
            return extractTitle(d).toLowerCase() == targetTitle;
        });

    if (articles.length === 0) {
        console.log("error decoding...");
        return "";
    }

    let d = articles[0];
    let title = extractTitle(d);
    let articlePageMatch = d.match(/<text>/);
    if (!articlePageMatch) {
        console.log("error decoding...");
        return "";
    }
    let startPageContentIdx = articlePageMatch.index + articlePageMatch[0].length;
    let endPageContentIdx = d.slice(startPageContentIdx).indexOf("</text>")
    d = d.slice(startPageContentIdx, endPageContentIdx);

    d = d
        .replace(/&lt;ref[\s\S]{0,500}?&lt;\/ref&gt;/gi, "")
        .replace(/&lt;ref[\s\S]{0,500}?\/&gt;/gi, "")
        .replace(/&lt;ref&gt;[\s\S]{0,500}?&lt;\/ref&gt;/gi, "")
        .replace(/&lt;![\s\S]{0,500}?--&gt;/gi, "");

    return {
        "wikiText": d,
        "title": title
    };
}
function postProcessWikiHTML(wikiHTML, title) {
    wikiHTML = wikiHTML.replace(/<img.*?\/>/g, "");
    wikiHTML = "<h2 class=\"title\">"+title+"</h2>" + wikiHTML
    return wikiHTML;
}

function resultToHtml(result, title) {
    let decompressedData = bz2.decompress(result);
    let wikiText = new TextDecoder("utf-8").decode(decompressedData);
    let processedData = preprocessWikiText(wikiText, title);
    let wikiHTML = wtf(processedData.wikiText).html();
    wikiHTML = postProcessWikiHTML(wikiHTML, processedData.title);
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
    document.querySelector('.searchbutton').insertAdjacentHTML('afterend', htmlSuggestions);
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
    return title && window.title_index.hasOwnProperty(title) && window.title_index[title] < window.numArticles;
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

function startLoading(message, hasProgress) {
    window.loading = true;
    window.started_loading = Date.now();
    if (hasProgress) {
        document.querySelector(".progress").classList.remove("off");
        document.querySelector(".loading-icon").classList.add("off");
    } else {
        document.querySelector(".progress").classList.add("off");
        document.querySelector(".loading-icon").classList.remove("off");
        document.querySelector(".loading-icon").classList.remove("hidden");
    }
    document.querySelector(".loading .message").innerHTML = message+"...";
    document.querySelector(".loading .message").classList.add("inprogress");
}

function stopLoading(message) {
    window.loading = false;
    document.querySelector(".loading-icon").classList.add("hidden");
    let seconds = (Date.now() - window.started_loading) / 1000
    let secondsRounded = Math.round(seconds * 100) / 100;
    let timingMessage = secondsRounded > 0.01 ? (" Took "+secondsRounded+"s.") : "";
    document.querySelector(".loading .message").innerHTML = "Done " + message.toLowerCase() + "." + timingMessage;
    document.querySelector(".loading .message").classList.remove("inprogress");
}

function queryTitleOnClick(title, displayTitle) {
    return async (e) => {
        e.preventDefault();
        document.querySelector(".searchbox").value = displayTitle;
        window.scrollTo(0, 0);
        queryTitle(title);
        return false;
    }
}

function enableLinks(element) {
    element.querySelectorAll('a').forEach((el) => {
        let displayTitle = el.getAttribute("href").slice(2).replace(/_/g, " ");
        let linkTitle = displayTitle.toLowerCase();
        if (hasTitle(linkTitle)) {
            el.onclick = queryTitleOnClick(linkTitle, displayTitle);
        } else {
            var redirected = followRedirects(linkTitle);
            if (redirected !== null && hasTitle(redirected)) {
                el.onclick = queryTitleOnClick(redirected, displayTitle);
            } else {
                el.classList.add("broken")
            }
        }
    })
}

const DB_NAME = 'spiralKey';
const KEY_SIZE = 32;
const MAX_VALID_TIME = 604800000; // 1 week

async function arrayBufferToBase64(data) {
    const base64url = await new Promise((r) => {
        const reader = new FileReader()
        reader.onload = () => r(reader.result)
        reader.readAsDataURL(new Blob([data]))
    })
    return base64url.split(",", 2)[1]
}

function base64ToArrayBuffer(str) {
    return Uint8Array.from(atob(str), c => c.charCodeAt(0));
}

async function storeState(key, uuid) {
    console.log(key);
    let dataToStore = {
        "key": await arrayBufferToBase64(key),
        "uuid": uuid,
        "createdAt": Date.now()
    }
    window.localStorage[DB_NAME] = JSON.stringify(dataToStore);
}

function retrieveState() {
    if (!window.localStorage || !window.localStorage[DB_NAME]) return false;
    let state = JSON.parse(window.localStorage[DB_NAME]);
    state["key"] = base64ToArrayBuffer(state["key"]);
    return state;
}

function setStateFromKey(key) {
    console.log("Initializing...");
    window.key = key;
    window.client = initialize(undefined, key);
    console.log("done");
    console.log("Generating public parameters...");
    window.publicParameters = generate_public_parameters(window.client);
    console.log(`done (${publicParameters.length} bytes)`);
}

async function isStateValid(state) {
    console.log("Checking if cached state is still valid")
    if (Date.now() - state.createdAt > MAX_VALID_TIME) return false;
        
    let isValidResponse = await api.check(state.uuid);
    let isValid = isValidResponse.is_valid;
    if (!isValid) return false;

    return true;
}

async function setUpClient() {
    let state = retrieveState();
    if (state && await isStateValid(state)) {
        console.log("Loading previous client state")
        setStateFromKey(state.key);
        window.id = state.uuid;
        return true;
    } else {
        console.log("No state stored, generating new client state")
        let key = new Uint8Array(KEY_SIZE);
        self.crypto.getRandomValues(key);
        setStateFromKey(key);
        return false;
    }
}

async function uploadState() {
    startLoading("Uploading setup data", true);
    console.log("Sending public parameters...");
    let setup_resp = await api.setup(window.publicParameters);
    console.log("sent.");
    let id = setup_resp["id"];
    stopLoading("Uploading setup data");
    await storeState(window.key, id);
    return id;
}

async function query(targetIdx, title) {
    if (!window.hasSetUp) {
        let id = await uploadState();
        if (!id) return false;
        window.hasSetUp = true;
        window.id = id;
    }

    startLoading("Loading article");
    console.log("Generating query... ("+targetIdx+")");
    let query = generate_query(window.client, window.id, targetIdx);
    console.log(`done (${query.length} bytes)`);

    console.log("Sending query...");
    let response = new Uint8Array(await api.query(query));
    console.log("sent.");

    console.log(`done, got (${response.length} bytes)`);

    console.log("Decoding result...");
    let result = decode_response(window.client, response)
    console.log("done.")
    console.log("Final result:")
    console.log(result);

    let resultHtml = resultToHtml(result, title);

    let outputArea = document.getElementById("output");
    outputArea.innerHTML = resultHtml;

    enableLinks(outputArea);
    stopLoading("Loading article");
}

async function queryTitle(targetTitle) {
    let redirectedTitle = followRedirects(targetTitle);
    let articleIndex = window.title_index[redirectedTitle];
    return await query(articleIndex, targetTitle);
}

async function run() {
    startLoading("Initializing");
    await init();
    stopLoading("Initializing");

    window.numArticles = 65536;
    window.articleSize = 100000;

    let makeQueryBtn = document.querySelector('#make_query');
    let searchBox = document.querySelector(".searchbox");
    document.querySelector(".sidebar-collapse-btn").onclick = () => {
        document.querySelector(".sidebar").classList.toggle("collapsed");
    }

    startLoading("Loading article titles");
    let title_index_p = getData("data/enwiki-20220320-index.json", true);
    let redirect_backlinks_p = getData("data/redirects-old.json", true);
    let setupClientResult = setUpClient();

    window.title_index = await title_index_p;
    let keys = Object.keys(window.title_index);
    for (var i = 0; i < keys.length; i++) {
        let key = keys[i];
        window.title_index[key] /= window.articleSize; 
        window.title_index[key.toLowerCase()] = window.title_index[key];
    }
    let redirect_backlinks = await redirect_backlinks_p;
    keys = Object.keys(redirect_backlinks);
    window.redirects = {}
    for (var i = 0; i < keys.length; i++) {
        let redirect_dest = keys[i];
        let redirect_srcs = redirect_backlinks[redirect_dest];
        for (var j = 0; j < redirect_srcs.length; j++) {
            window.redirects[redirect_srcs[j].toLowerCase()] = redirect_dest;
        }
    }

    window.hasSetUp = await setupClientResult;
    stopLoading("Loading article titles");

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

function setProgress(progress) {
    document.querySelector(".progress").style.background =
        "conic-gradient(#333 " +
        progress +
        "%,#fff " +
        progress +
        "%)";

    // document.getElementById("middle-circle").innerHTML =
    //     progress.toString() + "%";
}
window.setProgress = setProgress;