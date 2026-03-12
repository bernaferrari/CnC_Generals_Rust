#include <cstdlib> // for FILE ops
#include "global.h"

GlobalClass Global;

GlobalClass::GlobalClass(void)
{}

bool GlobalClass::ReadFile(const char *fname)
{
	FILE *fp;
	if ((fp = fopen(fname, "r")) == NULL)
		return false;
	config.readFile(fp);
	fclose(fp);

	return true;
}

bool GlobalClass::GetString(const Wstring& key, Wstring& val)
{
	val = "";
	config.getString(key, val, "STRINGS");
	if (val == "")
	{
		val.setFormatted("MISSING:%s", key.get());
		return false;
	}

	return true;

}

