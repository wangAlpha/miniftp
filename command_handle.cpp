// /* 访问控制命令 */
// void do_user(); /* 用户名 */
// void do_pass(); /* 密码  */
// void do_cwd();  /* 改变工作目录 */
// void do_cdup(); /* 返回上层目录 */
// void do_quit(); /* 注销用户 */
// /* 传输参数命令 */
// void do_port(); /* 指定数据连接时的主机数据端口 */
// void do_pasv(); /* 被动模式 */
// void do_type(); /* 表示类型 */
// /* FTP 服务命令 */
// void do_retr(); /* 获取某个文件 */
// void do_stor(); /* 保存（上传）文件 */
// void do_rest(); /* 重新开始（实现断点续传）*/
// void do_rnfr(); /* 重命名开始 */
// void do_rnto(); /* 重命名为（两个命令一起为文件重新命名）  */
// void do_abor(); /* 放弃 */
// void do_delete(); /* 删除文件 */
// void do_rmd();    /* 删除目录 */
// void do_mkd();    /* 新建目录 */
// void do_pwd();    /* 打印工作目录 */
// void do_list();   /* 列出文件信息 */
// void do_nlst();   /* 列出名字列表 */
// void do_site();   /* 站点参数 */
// void do_syst();   /* 操作系统类型 */
// void do_help();   /* 帮助信息 */
// void do_noop();   /* 空操作 */
// /* 其他 */
// void do_feat(); /* 处理Feat命令 */
// void do_opts(); /* 调整选项 */
// void do_size(); /* 获取文件的大小 */
// private:
// void do_chmod(unsigned int perm, const char *file_name);

// void do_unmask(unsigned int mask);

// ipc_utility::EMState do_send_list(bool verbose);

// std::shared_ptr<tcp::CLConnection> get_data_connection();

// enum EMMode {
//   ASCII,  /* ascii文本格式 */
//   BINARY, /* 二进制格式 */
// };
// EMMode m_data_type; /* 传输的数据类型 */

// std::string m_file_name; /* 用来记录需要重命名的文件名 */
// static const int BYTES_PEER_TRANSFER =
//     1024 * 100; /* 文件下载时每次传输的数据量：100k Bytes/s */
// long long m_resume_point; /* 断点续传点 */
// private:
// uid_t m_uid; /* 用户 id  */
