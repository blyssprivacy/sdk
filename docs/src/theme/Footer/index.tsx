import { useThemeConfig } from '@docusaurus/theme-common';
import FooterCopyright from '@theme/Footer/Copyright';
import FooterLayout from '@theme/Footer/Layout';
import React from 'react';

function Footer() {
  const { footer } = useThemeConfig();
  if (!footer) {
    return null;
  }
  let { copyright, style } = footer;

  const elem = (
    <div className="footer-wrapper">
      <div className="footer-logo-tagline">
        <div className="footer-logo">
          <div className="navbar__title">blyss</div>
        </div>
        <div className="footer-tagline">
          The next generation of privacy. Today. <br />
          &nbsp;
        </div>
        <div className="footer-copyright">© 2023 Blyss, Inc.</div>
      </div>
    </div>
  );
  // links={links && links.length > 0 && <FooterLinks links={links} />}
  return (
    <FooterLayout
      style={style}
      logo={elem}
      links={null}
      copyright={
        copyright && (
          <div style={{ fontSize: 8 }}>
            <FooterCopyright copyright={copyright} />
          </div>
        )
      }
    />
  );
}
export default React.memo(Footer);
