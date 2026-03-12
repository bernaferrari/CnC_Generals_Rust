#ifndef CONFIGFILE_HEADER
#define CONFIGFILE_HEADER

#include "dictionary.h"
#include "wstring.h"

class ConfigFile
{
 public:
             ConfigFile();
            ~ConfigFile();
 bit8        readFile(IN FILE *config);
 bit8        getString(IN Wstring &key,OUT Wstring &value);
 bit8        getString(IN char *key,OUT Wstring &value);

 bit8        getInt(IN Wstring &key,OUT sint32 &value);
 bit8        getInt(IN char *key,OUT sint32 &value);

 bit8        getInt(IN Wstring &key,OUT sint16 &value);
 bit8        getInt(IN char *key,OUT sint16 &value);

 private:
  Dictionary<Wstring,Wstring> dictionary; // stores the mappings from keys
                                        //  to value strings
};

#endif
