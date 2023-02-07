import WasmPackPlugin from '@wasm-tool/wasm-pack-plugin';
import path from 'path';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const dist = path.resolve(__dirname, 'dist');
const distLib = path.resolve(dist, 'lib');

const config = {
  name: 'web',
  mode: 'production',
  context: path.resolve(__dirname, 'js'),
  entry: {
    index: './index'
  },
  devtool: 'source-map',
  performance: {
    maxEntrypointSize: 512000,
    maxAssetSize: 512000
  },
  output: {
    path: dist,
    filename: 'index.js',
    chunkFormat: 'module',
    library: {
      type: 'module'
    }
  },
  module: {
    rules: [
      {
        test: /\.tsx?$/,
        use: {
          loader: 'ts-loader'
        },
        exclude: [/node_modules/, path.resolve(__dirname, 'examples')]
      },
      {
        test: /\.wasm$/,
        type: 'asset/inline'
      }
    ]
  },
  resolve: {
    extensions: ['.tsx', '.ts', '.js']
  },
  devServer: {
    static: dist
  },
  experiments: {
    asyncWebAssembly: true,
    outputModule: true
  },
  plugins: [
    new WasmPackPlugin({
      crateDirectory: path.resolve(__dirname, 'js/bridge'),
      outDir: distLib,
      extraArgs: '--target web',
      outName: 'lib',
      forceMode: 'production'
    })
  ],
  target: 'web'
};

const webTarget = config;

const webSingleFileTarget = {
  ...config,
  name: 'web-single-file',
  dependencies: ['web'],
  output: {
    path: dist,
    filename: 'blyss-bundle.min.js',
    library: {
      type: 'window',
      name: 'blyss'
    }
  }
};

export default [webTarget, webSingleFileTarget];
