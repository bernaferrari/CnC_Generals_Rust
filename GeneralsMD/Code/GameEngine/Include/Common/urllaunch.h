
#ifndef URLLAUNCH_H
#define URLLAUNCH_H

HRESULT MakeEscapedURL( LPWSTR pszInURL, LPWSTR *ppszOutURL );

HRESULT LaunchURL( LPCWSTR pszURL );

#endif  // URLLAUNCH_H
