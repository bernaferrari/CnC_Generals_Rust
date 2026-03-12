#include <string>
#include <stdio.h>
#include "registry.h"

void FormatURLFromRegistry( std::string& gamePatchURL, std::string& mapPatchURL,
													 std::string& configURL, std::string& motdURL )
{
	std::string sku = "GeneralsZH";
	std::string language = "english";
	unsigned int version = 0; // invalid version - can't get on with a corrupt reg.
	unsigned int mapVersion = 0; // invalid version - can't get on with a corrupt reg.
	std::string baseURL = "http://servserv.generals.ea.com/servserv/";
	baseURL.append(sku);
	baseURL.append("/");

	GetStringFromRegistry("", "BaseURL", baseURL);
	GetStringFromRegistry("", "Language", language);
	GetUnsignedIntFromRegistry("", "Version", version);
	GetUnsignedIntFromRegistry("", "MapPackVersion", mapVersion);

	char buf[256];
	_snprintf(buf, 256, "%s%s-%d.txt", baseURL.c_str(), language.c_str(), version);
	gamePatchURL = buf;
	_snprintf(buf, 256, "%smaps-%d.txt", baseURL.c_str(), mapVersion);
	mapPatchURL = buf;
	_snprintf(buf, 256, "%sconfig.txt", baseURL.c_str());
	configURL = buf;
	_snprintf(buf, 256, "%sMOTD-%s.txt", baseURL.c_str(), language.c_str());
	motdURL = buf;
}

