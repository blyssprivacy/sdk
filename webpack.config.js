import WasmPackPlugin from '@wasm-tool/wasm-pack-plugin';
import path from 'path';
import { fileURLToPath } from 'url';
import CopyPlugin from 'copy-webpack-plugin';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const dist = path.resolve(__dirname, 'dist');
const distLib = path.resolve(dist, 'lib');
const distCjs = path.resolve(dist, 'cjs');

const config = {
  name: 'web',
  mode: 'development',
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
  externals: {
    'node:crypto': 'commonjs2 node:crypto'
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

const nodeTarget = {
  ...config,
  name: 'node',
  dependencies: ['web'],
  output: {
    path: dist + '/cjs',
    filename: 'index.cjs',
    library: {
      type: 'commonjs'
    }
  },
  externals: {},
  experiments: {
    asyncWebAssembly: true
  },
  target: 'node',
  plugins: [
    new CopyPlugin({
      patterns: [{ from: '../cjs-package.json', to: distCjs + '/package.json' }]
    })
  ]
};
export default [webTarget, webSingleFileTarget, nodeTarget];
