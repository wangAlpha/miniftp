
void command_handler::open(const vector<string> &args) {
  if (ftp_client_.is_open()) {
    throw cmdline_exception("Already connected, use close first.");
  }

  string hostname;
  uint16_t port = 21;

  if (args.empty()) {
    hostname = utils::read_line("hostname: ");
  } else if (args.size() == 1) {
    hostname = args[0];
  } else if (args.size() == 2) {
    hostname = args[0];

    if (!boost::conversion::try_lexical_convert(args[1], port)) {
      throw cmdline_exception("Invalid port number.");
    }
  } else {
    throw cmdline_exception("usage: open hostname [ port ]");
  }

  bool ftp_result = ftp_client_.open(hostname, port);

  if (!ftp_result) {
    return;
  }

  string username = utils::read_line("username: ");
  string password = utils::read_password("password: ");

  ftp_result = ftp_client_.login(username, password);

  if (!ftp_result) {
    return;
  }

  /* Use binary mode to transfer files by default. */
  ftp_client_.binary();
}

void command_handler::user(const vector<string> &args) {
  string username;
  string password;

  if (args.empty()) {
    username = utils::read_line("username: ");
    password = utils::read_password("password: ");
  } else if (args.size() == 1) {
    username = args[0];
    password = utils::read_password("password: ");
  } else {
    throw cmdline_exception("usage: user username");
  }

  bool ftp_result = ftp_client_.login(username, password);

  if (!ftp_result) {
    return;
  }

  /* Use binary mode to transfer files by default. */
  ftp_client_.binary();
}

void command_handler::cd(const vector<string> &args) {
  string remote_directory;

  if (args.empty()) {
    remote_directory = utils::read_line("remote directory: ");
  } else if (args.size() == 1) {
    remote_directory = args[0];
  } else {
    throw cmdline_exception("usage: cd remote-directory");
  }

  ftp_client_.cd(remote_directory);
}

void command_handler::ls(const vector<string> &args) {
  if (args.empty()) {
    ftp_client_.ls();
  } else if (args.size() == 1) {
    ftp_client_.ls(args[0]);
  } else {
    throw cmdline_exception("usage: ls [ remote-directory ]");
  }
}

void command_handler::put(const vector<string> &args) {
  string local_file, remote_file;

  if (args.empty()) {
    local_file = utils::read_line("local-file: ");
    remote_file = utils::get_filename(local_file);
  } else if (args.size() == 1) {
    local_file = args[0];
    remote_file = utils::get_filename(local_file);
  } else if (args.size() == 2) {
    local_file = args[0];
    remote_file = args[1];
  } else {
    throw cmdline_exception("usage: put local-file [ remote-file ]");
  }

  ftp_client_.upload(local_file, remote_file);
}

void command_handler::get(const vector<string> &args) {
  string remote_file, local_file;

  if (args.empty()) {
    remote_file = utils::read_line("remote-file: ");
    local_file = utils::get_filename(remote_file);
  } else if (args.size() == 1) {
    remote_file = args[0];
    local_file = utils::get_filename(remote_file);
  } else if (args.size() == 2) {
    remote_file = args[0];
    local_file = args[1];
  } else {
    throw cmdline_exception("usage: get remote-file [ local-file ]");
  }

  ftp_client_.download(remote_file, local_file);
}

void command_handler::pwd() { ftp_client_.pwd(); }

void command_handler::mkdir(const vector<string> &args) {
  string directory_name;

  if (args.empty()) {
    directory_name = utils::read_line("directory-name: ");
  } else if (args.size() == 1) {
    directory_name = args[0];
  } else {
    throw cmdline_exception("usage: mkdir directory-name");
  }

  ftp_client_.mkdir(directory_name);
}

void command_handler::rmdir(const vector<string> &args) {
  string directory_name;

  if (args.empty()) {
    directory_name = utils::read_line("directory-name: ");
  } else if (args.size() == 1) {
    directory_name = args[0];
  } else {
    throw cmdline_exception("usage: rmdir directory-name");
  }

  ftp_client_.rmdir(directory_name);
}

void command_handler::del(const vector<string> &args) {
  string remote_file;

  if (args.empty()) {
    remote_file = utils::read_line("remote-file: ");
  } else if (args.size() == 1) {
    remote_file = args[0];
  } else {
    throw cmdline_exception("usage: del remote-file");
  }

  ftp_client_.rm(remote_file);
}

void command_handler::binary() { ftp_client_.binary(); }

void command_handler::size(const vector<string> &args) {
  string remote_file;

  if (args.empty()) {
    remote_file = utils::read_line("remote-file: ");
  } else if (args.size() == 1) {
    remote_file = args[0];
  } else {
    throw cmdline_exception("usage: size remote-file");
  }

  ftp_client_.size(remote_file);
}

void command_handler::stat(const vector<string> &args) {
  if (args.empty()) {
    ftp_client_.stat();
  } else if (args.size() == 1) {
    ftp_client_.stat(args[0]);
  } else {
    throw cmdline_exception("usage: stat [ remote-file ]");
  }
}

void command_handler::syst() { ftp_client_.system(); }

void command_handler::noop() { ftp_client_.noop(); }

void command_handler::close() { ftp_client_.close(); }

void command_handler::exit() {
  if (ftp_client_.is_open()) {
    ftp_client_.close();
  }
}
