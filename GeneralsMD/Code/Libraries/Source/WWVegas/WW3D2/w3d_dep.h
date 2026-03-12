#ifndef W3D_DEP_H
#define W3D_DEP_H

#pragma warning (push, 3)
#pragma warning (disable: 4018 4284 4786 4788)
#include <list>
#pragma warning (pop)

#pragma warning (push, 3)
#pragma warning (disable: 4018 4146 4284 4503)
#include <strstream>
#include <string>
#pragma warning (pop)

typedef std::list<std::string>	StringList;
bool Get_W3D_Dependencies (const char *w3d_filename, StringList &files);

#endif
